/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use elp_base_db::FileId;
pub use elp_syntax::ast::BinaryOp;
pub use elp_syntax::ast::MapOp;
pub use elp_syntax::ast::UnaryOp;
use elp_syntax::SmolStr;
use la_arena::Idx;

use crate::sema;
use crate::Atom;
use crate::Body;
use crate::FunctionDef;
use crate::InFunctionBody;
use crate::RecordFieldId;
use crate::Semantic;
use crate::Var;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AnyExprId {
    Expr(ExprId),
    Pat(PatId),
    TypeExpr(TypeExprId),
    Term(TermId),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AnyExprRef<'a> {
    Expr(&'a Expr),
    Pat(&'a Pat),
    TypeExpr(&'a TypeExpr),
    Term(&'a Term),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Literal {
    String(String),
    Char(char),
    Atom(Atom),
    Integer(i128), // TODO: bigints
    Float(u64),    // FIXME: f64 is not Eq
}

impl Literal {
    pub fn negate(&self) -> Option<Self> {
        match self {
            Literal::String(_) => None,
            Literal::Atom(_) => None,
            // Weird, but allowed https://github.com/erlang/otp/blob/09c601fa2183d4c545791ebcd68f869a5ab912a4/lib/stdlib/src/erl_parse.yrl#L1432
            Literal::Char(ch) => Some(Literal::Integer(-(*ch as i128))),
            Literal::Integer(int) => Some(Literal::Integer(-int)),
            Literal::Float(bits) => Some(Literal::Float((-f64::from_bits(*bits)).to_bits())),
        }
    }
}

pub type ExprId = Idx<Expr>;

#[derive(Debug, Clone, Eq, PartialEq)]
/// A regular Erlang expression
pub enum Expr {
    /// This is produced if the syntax tree does not have a required
    /// expression piece, or it was in some way invalid
    Missing,
    Literal(Literal),
    Var(Var),
    Match {
        lhs: PatId,
        rhs: ExprId,
    },
    Tuple {
        exprs: Vec<ExprId>,
    },
    List {
        exprs: Vec<ExprId>,
        tail: Option<ExprId>,
    },
    Binary {
        segs: Vec<BinarySeg<ExprId>>,
    },
    UnaryOp {
        expr: ExprId,
        op: UnaryOp,
    },
    BinaryOp {
        lhs: ExprId,
        rhs: ExprId,
        op: BinaryOp,
    },
    Record {
        name: Atom,
        fields: Vec<(Atom, ExprId)>,
    },
    RecordUpdate {
        expr: ExprId,
        name: Atom,
        fields: Vec<(Atom, ExprId)>,
    },
    RecordIndex {
        name: Atom,
        field: Atom,
    },
    RecordField {
        expr: ExprId,
        name: Atom,
        field: Atom,
    },
    Map {
        fields: Vec<(ExprId, ExprId)>,
    },
    MapUpdate {
        expr: ExprId,
        fields: Vec<(ExprId, MapOp, ExprId)>,
    },
    Catch {
        expr: ExprId,
    },
    MacroCall {
        // This constructor captures the point a macro is expanded
        // into an expression. This allows us to separately track the
        // arguments, for things like highlight related, or unused
        // function arguments.
        expansion: ExprId,
        args: Vec<ExprId>,
    },
    Call {
        target: CallTarget<ExprId>,
        args: Vec<ExprId>,
    },
    Comprehension {
        builder: ComprehensionBuilder,
        exprs: Vec<ComprehensionExpr>,
    },
    Block {
        exprs: Vec<ExprId>,
    },
    If {
        clauses: Vec<IfClause>,
    },
    Case {
        expr: ExprId,
        clauses: Vec<CRClause>,
    },
    Receive {
        clauses: Vec<CRClause>,
        after: Option<ReceiveAfter>,
    },
    Try {
        exprs: Vec<ExprId>,
        of_clauses: Vec<CRClause>,
        catch_clauses: Vec<CatchClause>,
        after: Vec<ExprId>,
    },
    CaptureFun {
        target: CallTarget<ExprId>,
        arity: ExprId,
    },
    Closure {
        clauses: Vec<Clause>,
        name: Option<PatId>,
    },
    Maybe {
        exprs: Vec<MaybeExpr>,
        else_clauses: Vec<CRClause>,
    },
}

impl Expr {
    pub fn as_atom(&self) -> Option<Atom> {
        match self {
            Expr::Literal(Literal::Atom(atom)) => Some(*atom),
            _ => None,
        }
    }

    pub fn as_var(&self) -> Option<Var> {
        match self {
            Expr::Var(var) => Some(*var),
            _ => None,
        }
    }

    pub fn list_length(&self) -> Option<usize> {
        match &self {
            Expr::List { exprs, tail } => {
                // Deal with a simple list only.
                if tail.is_some() {
                    None
                } else {
                    Some(exprs.len())
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MaybeExpr {
    Cond { lhs: PatId, rhs: ExprId },
    Expr(ExprId),
}

pub type ClauseId = Idx<Clause>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Clause {
    pub pats: Vec<PatId>,
    pub guards: Vec<Vec<ExprId>>,
    pub exprs: Vec<ExprId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CRClause {
    pub pat: PatId,
    pub guards: Vec<Vec<ExprId>>,
    pub exprs: Vec<ExprId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IfClause {
    pub guards: Vec<Vec<ExprId>>,
    pub exprs: Vec<ExprId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CatchClause {
    pub class: Option<PatId>,
    pub reason: PatId,
    pub stack: Option<PatId>,
    pub guards: Vec<Vec<ExprId>>,
    pub exprs: Vec<ExprId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordFieldBody {
    pub field_id: RecordFieldId,
    pub expr: Option<ExprId>,
    pub ty: Option<TypeExprId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReceiveAfter {
    pub timeout: ExprId,
    pub exprs: Vec<ExprId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CallTarget<Id> {
    Local { name: Id },
    Remote { module: Id, name: Id },
}

impl CallTarget<ExprId> {
    pub fn resolve_call(
        &self,
        arity: u32,
        sema: &Semantic,
        file_id: FileId,
        body: &Body,
    ) -> Option<FunctionDef> {
        sema::to_def::resolve_call_target(sema, self, arity, file_id, body)
    }

    pub fn label(&self, arity: u32, sema: &Semantic, body: &Body) -> Option<SmolStr> {
        match self {
            CallTarget::Local { name } => {
                let name = sema.db.lookup_atom(body[*name].as_atom()?);
                Some(SmolStr::new(format!("{name}/{arity}")))
            }
            CallTarget::Remote { module, name } => {
                let name = sema.db.lookup_atom(body[*name].as_atom()?);
                let module = sema.db.lookup_atom(body[*module].as_atom()?);
                Some(SmolStr::new(format!("{module}:{name}/{arity}",)))
            }
        }
    }

    pub fn label_short(&self, sema: &Semantic, body: &Body) -> Option<SmolStr> {
        match self {
            CallTarget::Local { name } => {
                let name = sema.db.lookup_atom(body[*name].as_atom()?);
                Some(SmolStr::new(format!("{name}")))
            }
            CallTarget::Remote { module, name } => {
                let name = sema.db.lookup_atom(body[*name].as_atom()?);
                let module = sema.db.lookup_atom(body[*module].as_atom()?);
                Some(SmolStr::new(format!("{module}:{name}",)))
            }
        }
    }

    pub fn is_module_fun(
        &self,
        sema: &Semantic,
        def_fb: &InFunctionBody<&FunctionDef>,
        module_name: crate::Name,
        fun_name: crate::Name,
    ) -> bool {
        match self {
            CallTarget::Local { name: _ } => false,
            CallTarget::Remote { module, name } => {
                sema.is_atom_named(&def_fb[*module], module_name)
                    && sema.is_atom_named(&def_fb[*name], fun_name)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ComprehensionBuilder {
    List(ExprId),
    Binary(ExprId),
    Map(ExprId, ExprId),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ComprehensionExpr {
    BinGenerator {
        pat: PatId,
        expr: ExprId,
    },
    ListGenerator {
        pat: PatId,
        expr: ExprId,
    },
    MapGenerator {
        key: PatId,
        value: PatId,
        expr: ExprId,
    },
    Expr(ExprId),
}

pub type PatId = Idx<Pat>;

#[derive(Debug, Clone, Eq, PartialEq)]
/// A regular Erlang pattern
pub enum Pat {
    Missing,
    Literal(Literal),
    Var(Var),
    Match {
        lhs: PatId,
        rhs: PatId,
    },
    Tuple {
        pats: Vec<PatId>,
    },
    List {
        pats: Vec<PatId>,
        tail: Option<PatId>,
    },
    Binary {
        segs: Vec<BinarySeg<PatId>>,
    },
    UnaryOp {
        pat: PatId,
        op: UnaryOp,
    },
    BinaryOp {
        lhs: PatId,
        rhs: PatId,
        op: BinaryOp,
    },
    Record {
        name: Atom,
        fields: Vec<(Atom, PatId)>,
    },
    RecordIndex {
        name: Atom,
        field: Atom,
    },
    /// map keys in patterns are allowed to be a subset of expressions
    Map {
        fields: Vec<(ExprId, PatId)>,
    },
    MacroCall {
        // This constructor captures the point a macro is expanded
        // into an expression. This allows us to separately track the
        // arguments, for things like highlight related, or unused
        // function arguments.
        expansion: PatId,
        args: Vec<ExprId>,
    },
}

impl Pat {
    pub fn as_var(&self) -> Option<Var> {
        match self {
            Pat::Var(var) => Some(*var),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BinarySeg<Val> {
    pub elem: Val,
    pub size: Option<ExprId>,
    // TODO we might want to normalise this, but it's pretty complex
    // See logic in https://github.com/erlang/otp/blob/master/lib/stdlib/src/erl_bits.erl
    pub tys: Vec<Atom>,
    pub unit: Option<i128>,
}

impl<T> BinarySeg<T> {
    pub fn with_value<U>(&self, value: U) -> BinarySeg<U> {
        BinarySeg {
            elem: value,
            size: self.size,
            tys: self.tys.clone(),
            unit: self.unit,
        }
    }

    pub fn map<F: FnOnce(T) -> U, U>(self, f: F) -> BinarySeg<U> {
        BinarySeg {
            elem: f(self.elem),
            size: self.size,
            tys: self.tys,
            unit: self.unit,
        }
    }
}

pub type TermId = Idx<Term>;

#[derive(Debug, Clone, Eq, PartialEq)]
/// A limited expression translated as a constant term, e.g. in module attributes
pub enum Term {
    Missing,
    Literal(Literal),
    Binary(Vec<u8>),
    Tuple {
        exprs: Vec<TermId>,
    },
    List {
        exprs: Vec<TermId>,
        tail: Option<TermId>,
    },
    Map {
        fields: Vec<(TermId, TermId)>,
    },
    CaptureFun {
        module: Atom,
        name: Atom,
        arity: u32,
    },
    MacroCall {
        // This constructor captures the point a macro is expanded
        // into an expression. This allows us to separately track the
        // arguments, for things like highlight related, or unused
        // function arguments.
        expansion: TermId,
        args: Vec<ExprId>,
    },
}

pub type TypeExprId = Idx<TypeExpr>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TypeExpr {
    AnnType {
        var: Var,
        ty: TypeExprId,
    },
    BinaryOp {
        lhs: TypeExprId,
        rhs: TypeExprId,
        op: BinaryOp,
    },
    Call {
        target: CallTarget<TypeExprId>,
        args: Vec<TypeExprId>,
    },
    Fun(FunType),
    List(ListType),
    Literal(Literal),
    Map {
        fields: Vec<(TypeExprId, MapOp, TypeExprId)>,
    },
    Missing,
    Union {
        types: Vec<TypeExprId>,
    },
    Range {
        lhs: TypeExprId,
        rhs: TypeExprId,
    },
    Record {
        name: Atom,
        fields: Vec<(Atom, TypeExprId)>,
    },
    Tuple {
        args: Vec<TypeExprId>,
    },
    UnaryOp {
        type_expr: TypeExprId,
        op: UnaryOp,
    },
    Var(Var),
    MacroCall {
        // This constructor captures the point a macro is expanded
        // into an expression. This allows us to separately track the
        // arguments, for things like highlight related, or unused
        // function arguments.
        expansion: TypeExprId,
        args: Vec<ExprId>,
    },
}

impl TypeExpr {
    pub fn as_atom(&self) -> Option<Atom> {
        match self {
            TypeExpr::Literal(Literal::Atom(atom)) => Some(*atom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FunType {
    Any,
    AnyArgs {
        result: TypeExprId,
    },
    Full {
        params: Vec<TypeExprId>,
        result: TypeExprId,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ListType {
    Empty,
    Regular(TypeExprId),
    NonEmpty(TypeExprId),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SpecSig {
    pub args: Vec<TypeExprId>,
    pub result: TypeExprId,
    pub guards: Vec<(Var, TypeExprId)>,
}
