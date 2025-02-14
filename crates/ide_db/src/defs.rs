/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

//! This implements the "go to definiton" logic

use std::iter;

use either::Either;
use elp_base_db::FileId;
use elp_syntax::ast;
use elp_syntax::match_ast;
use elp_syntax::AstNode;
use elp_syntax::SmolStr;
use elp_syntax::SyntaxNode;
use elp_syntax::SyntaxToken;
use hir::db::MinDefDatabase;
use hir::CallDef;
use hir::CallbackDef;
use hir::DefineDef;
use hir::DefinitionOrReference;
use hir::FaDef;
use hir::File;
use hir::FunctionDef;
use hir::InFile;
use hir::Module;
use hir::RecordDef;
use hir::RecordFieldDef;
use hir::Semantic;
use hir::TypeAliasDef;
use hir::VarDef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolClass {
    Definition(SymbolDefinition),
    Reference {
        refs: ReferenceClass,
        typ: ReferenceType,
    },
    // Operator(...)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceType {
    Direct,
    Other, // spec, import, export
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceClass {
    Definition(SymbolDefinition),
    /// A variable defined in multiple places, e.g. after a case
    /// for a variable defined in all branches
    MultiVar(Vec<VarDef>),
    /// An arity-less reference to a macro, can refer to multiple definitions
    MultiMacro(Vec<DefineDef>),
}

impl ReferenceClass {
    pub fn into_iter(self) -> impl Iterator<Item = SymbolDefinition> {
        match self {
            ReferenceClass::Definition(def) => Either::Left(iter::once(def)),
            ReferenceClass::MultiVar(vars) => {
                Either::Right(Either::Left(vars.into_iter().map(SymbolDefinition::Var)))
            }
            ReferenceClass::MultiMacro(defs) => Either::Right(Either::Right(
                defs.into_iter().map(SymbolDefinition::Define),
            )),
        }
    }
}

/// `SymbolDefinition` keeps information about the element we want to search references for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolDefinition {
    Module(Module),
    Function(FunctionDef),
    Record(RecordDef),
    RecordField(RecordFieldDef),
    Type(TypeAliasDef),
    Callback(CallbackDef),
    Define(DefineDef),
    Header(File),
    Var(VarDef),
}

impl SymbolClass {
    /// Returns the SymbolClass for the token, if:
    /// * it is reference place for the definition, e.g. a function call for a function
    /// * it is a definition itself, e.g. a function definition
    pub fn classify(sema: &Semantic, token: InFile<SyntaxToken>) -> Option<SymbolClass> {
        let wrapper = token.value.parent()?;
        let parent = wrapper.parent()?;

        fn definition<Def: Into<SymbolDefinition>>(def: Option<Def>) -> Option<SymbolClass> {
            def.map(|def| SymbolClass::Definition(def.into()))
        }

        match_ast! {
            match parent {
                // All places that embed $._name
                ast::ModuleAttribute(attr) => {
                    definition(sema.to_def(token.with_value(&attr)))
                },
                ast::BehaviourAttribute(behaviour) => {
                    reference_direct(sema.to_def(token.with_value(&behaviour)))
                },
                ast::ImportAttribute(import) => {
                    reference_other(sema.to_def(token.with_value(&import)))
                },
                ast::Fa(fa) => {
                    reference_other(sema.to_def(token.with_value(&fa)))
                },
                ast::TypeName(ty) => {
                    definition(sema.to_def(token.with_value(&ty)))
                },
                ast::RecordDecl(rec) => {
                    definition(sema.to_def(token.with_value(&rec)))
                },
                ast::Spec(spec) => {
                    reference_other(sema.to_def(token.with_value(&spec)))
                },
                ast::Callback(cb) => {
                    definition(sema.to_def(token.with_value(&cb)))
                },
                ast::Module(_) => {
                    if let Some(atom) = ast::Atom::cast(wrapper.clone()) {
                        reference_direct(sema.to_def(token.with_value(&atom)))
                    } else {
                        classify_var(sema, token.file_id, wrapper)
                    }
                },
                ast::AttrName(_) => None,
                ast::FunctionClause(clause) => {
                    definition(sema.to_def(token.with_value(&clause)))
                },
                ast::BitTypeList(_) => None,
                ast::RecordName(name) => {
                    reference_direct(sema.to_def(token.with_value(&name)))
                },
                ast::RecordFieldName(field) => {
                    reference_direct(sema.to_def(token.with_value(&field)))
                },
                ast::RecordField(field) => {
                    match sema.to_def(token.with_value(&field))? {
                        DefinitionOrReference::Definition(def) => definition(Some(def)),
                        DefinitionOrReference::Reference(def) => reference_direct(Some(def)),
                    }
                },
                ast::InternalFun(fun) => {
                    if let Some(function) = sema.to_def(token.with_value(&fun)) {
                        reference_direct(Some(function))
                    } else {
                        classify_var(sema, token.file_id, wrapper)
                    }
                },
                ast::ExternalFun(fun) => {
                    if let Some(function) = sema.to_def(token.with_value(&fun)) {
                        reference_direct(Some(function))
                    } else {
                        classify_var(sema, token.file_id, wrapper)
                    }
                },
                ast::TryClass(_) => {
                    classify_var(sema, token.file_id, wrapper)
                },
                // All places that embed $._macro_name
                ast::MacroLhs(define) => {
                    definition(sema.to_def(token.with_value(&define)))
                },
                ast::MacroCallExpr(macro_call) => {
                    reference_direct(sema.to_def(token.with_value(&macro_call)))
                },
                ast::PpUndef(_) => {
                    classify_macro_name(sema, token.file_id, wrapper)
                },
                ast::PpIfdef(_) => {
                    classify_macro_name(sema, token.file_id, wrapper)
                },
                ast::PpIfndef(_) => {
                    classify_macro_name(sema, token.file_id, wrapper)
                },
                // All places that embed $._expr with special meaning
                ast::RemoteModule(_) => {
                    from_wrapper(sema, &token, wrapper)
                },
                ast::Remote(remote) => {
                    if let Some(call) = sema.to_def(token.with_value(&remote)) {
                        reference_direct(Some(call))
                    } else {
                        classify_var(sema, token.file_id, wrapper)
                    }
                },
                ast::Call(call) => {
                    if let Some(call) = sema.to_def(token.with_value(&call)) {
                        reference_direct(Some(call))
                    } else {
                        classify_var(sema, token.file_id, wrapper)
                    }
                },
                ast::PpInclude(include) => {
                    reference_direct(sema.to_def(token.with_value(&include)))
                },
                ast::PpIncludeLib(include) => {
                    reference_direct(sema.to_def(token.with_value(&include)))
                },
                ast::ExprArgs(args) => {
                    from_apply(sema, &token, args.syntax())
                        .or_else(|| from_wrapper(sema, &token, wrapper))
                },
                _ => {
                    from_wrapper(sema, &token, wrapper)
                }
            }
        }
    }

    pub fn into_iter(self) -> impl Iterator<Item = SymbolDefinition> {
        match self {
            SymbolClass::Definition(def) => Either::Left(iter::once(def)),
            SymbolClass::Reference { refs, typ: _ } => Either::Right(refs.into_iter()),
        }
    }
}

impl SymbolDefinition {
    pub fn file(&self) -> &File {
        match self {
            SymbolDefinition::Module(it) => &it.file,
            SymbolDefinition::Function(it) => &it.file,
            SymbolDefinition::Record(it) => &it.file,
            SymbolDefinition::RecordField(it) => &it.record.file,
            SymbolDefinition::Type(it) => &it.file,
            SymbolDefinition::Callback(it) => &it.file,
            SymbolDefinition::Define(it) => &it.file,
            SymbolDefinition::Header(it) => it,
            SymbolDefinition::Var(it) => &it.file,
        }
    }

    pub fn search_name(&self, db: &dyn MinDefDatabase) -> SmolStr {
        match self {
            SymbolDefinition::Module(it) => it.name(db).raw(),
            SymbolDefinition::Function(it) => it.function.name.name().raw(),
            SymbolDefinition::Record(it) => it.record.name.raw(),
            SymbolDefinition::RecordField(it) => it.field.name.raw(),
            SymbolDefinition::Type(it) => it.name().name().raw(),
            SymbolDefinition::Callback(it) => it.callback.name.name().raw(),
            SymbolDefinition::Define(it) => it.define.name.name().raw(),
            SymbolDefinition::Header(it) => it.name(db.upcast()),
            SymbolDefinition::Var(it) => it.name(db.upcast()).raw(),
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            SymbolDefinition::Function(fun) => !fun.exported,
            SymbolDefinition::Record(_) => true,
            SymbolDefinition::RecordField(_) => true,
            SymbolDefinition::Type(ty) => !ty.exported,
            SymbolDefinition::Callback(_) => true,
            SymbolDefinition::Define(_) => true,
            SymbolDefinition::Var(_) => true,
            SymbolDefinition::Module(_) => false,
            SymbolDefinition::Header(_) => false,
        }
    }
}

impl From<Module> for SymbolDefinition {
    fn from(it: Module) -> Self {
        Self::Module(it)
    }
}

impl From<TypeAliasDef> for SymbolDefinition {
    fn from(it: TypeAliasDef) -> Self {
        Self::Type(it)
    }
}

impl From<RecordDef> for SymbolDefinition {
    fn from(it: RecordDef) -> Self {
        Self::Record(it)
    }
}

impl From<RecordFieldDef> for SymbolDefinition {
    fn from(it: RecordFieldDef) -> Self {
        Self::RecordField(it)
    }
}

impl From<FunctionDef> for SymbolDefinition {
    fn from(it: FunctionDef) -> Self {
        Self::Function(it)
    }
}

impl From<CallbackDef> for SymbolDefinition {
    fn from(it: CallbackDef) -> Self {
        Self::Callback(it)
    }
}

impl From<DefineDef> for SymbolDefinition {
    fn from(it: DefineDef) -> Self {
        Self::Define(it)
    }
}

impl From<File> for SymbolDefinition {
    fn from(it: File) -> Self {
        Self::Header(it)
    }
}

impl From<FaDef> for SymbolDefinition {
    fn from(it: FaDef) -> Self {
        match it {
            FaDef::Function(function) => function.into(),
            FaDef::Type(alias) => alias.into(),
            FaDef::Callback(cb) => cb.into(),
        }
    }
}

impl From<CallDef> for SymbolDefinition {
    fn from(it: CallDef) -> Self {
        match it {
            CallDef::Function(function) => function.into(),
            CallDef::Type(alias) => alias.into(),
        }
    }
}

fn classify_var(sema: &Semantic, file_id: FileId, wrapper: SyntaxNode) -> Option<SymbolClass> {
    let var = ast::Var::cast(wrapper)?;
    match sema.to_def(InFile::new(file_id, &var))? {
        DefinitionOrReference::Definition(def) => {
            Some(SymbolClass::Definition(SymbolDefinition::Var(def)))
        }
        DefinitionOrReference::Reference(mut vars) if vars.len() == 1 => {
            Some(SymbolClass::Reference {
                refs: ReferenceClass::Definition(SymbolDefinition::Var(vars.swap_remove(0))),
                typ: ReferenceType::Direct,
            })
        }
        DefinitionOrReference::Reference(vars) => Some(SymbolClass::Reference {
            refs: ReferenceClass::MultiVar(vars),
            typ: ReferenceType::Direct,
        }),
    }
}

fn classify_macro_name(
    sema: &Semantic,
    file_id: FileId,
    wrapper: SyntaxNode,
) -> Option<SymbolClass> {
    let name = ast::MacroName::cast(wrapper)?;
    let mut defs = sema.to_def(InFile::new(file_id, &name))?;
    if defs.len() == 1 {
        Some(SymbolClass::Reference {
            refs: ReferenceClass::Definition(SymbolDefinition::Define(defs.swap_remove(0))),
            typ: ReferenceType::Direct,
        })
    } else {
        Some(SymbolClass::Reference {
            refs: ReferenceClass::MultiMacro(defs),
            typ: ReferenceType::Direct,
        })
    }
}

fn reference_direct<Def: Into<SymbolDefinition>>(def: Option<Def>) -> Option<SymbolClass> {
    def.map(|def| SymbolClass::Reference {
        refs: ReferenceClass::Definition(def.into()),
        typ: ReferenceType::Direct,
    })
}

fn reference_other<Def: Into<SymbolDefinition>>(def: Option<Def>) -> Option<SymbolClass> {
    def.map(|def| SymbolClass::Reference {
        refs: ReferenceClass::Definition(def.into()),
        typ: ReferenceType::Other,
    })
}

pub fn from_apply(
    sema: &Semantic,
    token: &InFile<SyntaxToken>,
    syntax: &SyntaxNode,
) -> Option<SymbolClass> {
    let call = ast::Call::cast(syntax.parent()?)?;
    let call_def = reference_direct(sema.to_def(token.with_value(&call.args()?)))?;
    match call_def {
        SymbolClass::Reference {
            refs: ReferenceClass::Definition(def),
            typ: _,
        } => reference_other(Some(def)),
        _ => None,
    }
}

/// Parent is nothing structured, it must be a raw atom or var literal
pub fn from_wrapper(
    sema: &Semantic,
    token: &InFile<SyntaxToken>,
    wrapper: SyntaxNode,
) -> Option<SymbolClass> {
    // Parent is nothing structured, it must be a raw atom or var literal
    if let Some(atom) = ast::Atom::cast(wrapper.clone()) {
        return reference_direct(sema.to_def(token.with_value(&atom)));
    } else {
        classify_var(sema, token.file_id, wrapper)
    }
}
