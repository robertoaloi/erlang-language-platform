/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use elp_syntax;
use elp_syntax::algo;
use elp_syntax::ast;
use elp_syntax::match_ast;
use elp_syntax::ted::Element;
use elp_syntax::AstNode;
use elp_syntax::NodeOrToken;
use elp_syntax::SyntaxElement;
use elp_syntax::SyntaxKind;
use elp_syntax::SyntaxNode;
use elp_syntax::SyntaxToken;
use elp_syntax::TextSize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ctx {
    Expr,
    Type,
    Export,
    ExportType,
    Other,
}

impl Ctx {
    pub fn new(node: &SyntaxNode, offset: TextSize) -> Self {
        if Self::is_atom_colon(node, offset) && Self::is_expr(node, offset) {
            Self::Expr
        } else if Self::is_export(node, offset) {
            Self::Export
        } else if Self::is_export_type(node, offset) {
            Self::ExportType
        } else if Self::is_attribute(node, offset) {
            Self::Other
        } else if Self::is_type_level_param(node, offset) || Self::is_pattern(node, offset) {
            Self::Other
        } else if Self::is_type(node, offset) {
            Self::Type
        } else if Self::is_expr(node, offset) {
            Self::Expr
        } else if Self::is_pp_define(node, offset) {
            Self::Expr
        } else {
            Self::Other
        }
    }
    fn is_atom_colon(node: &SyntaxNode, offset: TextSize) -> bool {
        // Temporary for T153426323
        let _pctx = stdx::panic_context::enter(format!("\nis_atom_colon"));
        if let Some(parent) = algo::ancestors_at_offset(node, offset).next() {
            match_ast! {
                    match parent {
                        ast::RemoteModule(_) => {
                            true
                        },
                        _ => false
                    }
            }
        } else {
            false
        }
    }
    fn is_export(node: &SyntaxNode, offset: TextSize) -> bool {
        algo::find_node_at_offset::<ast::ExportAttribute>(node, offset).is_some()
    }
    fn is_export_type(node: &SyntaxNode, offset: TextSize) -> bool {
        algo::find_node_at_offset::<ast::ExportTypeAttribute>(node, offset).is_some()
    }
    fn is_pp_define(node: &SyntaxNode, offset: TextSize) -> bool {
        algo::find_node_at_offset::<ast::PpDefine>(node, offset).is_some()
    }
    fn is_attribute(_node: &SyntaxNode, _offset: TextSize) -> bool {
        false
    }
    fn is_type_level_param(node: &SyntaxNode, offset: TextSize) -> bool {
        let head_opt = algo::find_node_at_offset::<ast::TypeAlias>(node, offset)
            .and_then(|type_alias| type_alias.name())
            .or(algo::find_node_at_offset::<ast::Opaque>(node, offset)
                .and_then(|opaque| opaque.name()));
        head_opt
            .map(|head| offset <= head.syntax().text_range().end())
            .unwrap_or_default()
    }
    fn is_pattern(node: &SyntaxNode, offset: TextSize) -> bool {
        // Temporary for T153426323
        let _pctx = stdx::panic_context::enter(format!("\nis_pattern"));
        algo::ancestors_at_offset(node, offset).any(|n| {
            let is_match = |node: &SyntaxNode| node.text_range() == n.text_range();
            if let Some(parent) = n.parent() {
                match_ast! {
                        match parent {
                            ast::CatchClause(parent) => {
                                if let Some(it) = parent.pat() {
                                    return is_match(it.syntax())
                                }
                            },
                            ast::FunClause(parent) => {
                                if let Some(it) = parent.args() {
                                    return is_match(it.syntax())
                                }
                            },
                            ast::FunctionClause(parent) => {
                                if let Some(it) = parent.args() {
                                    return is_match(it.syntax())
                                }
                            },
                            ast::MatchExpr(parent) => {
                                let prev_token = Self::previous_non_trivia_sibling_or_token(parent.syntax());
                                if Self::is_in_error(node, offset) {
                                    if let Some(NodeOrToken::Token(token)) = prev_token {
                                        if token.kind() == SyntaxKind::ANON_CASE {
                                            return false;
                                        }
                                    }
                                }
                                if let Some(it) = parent.lhs() {
                                    return is_match(it.syntax())
                                }
                            },
                            ast::CrClause(parent) => {
                                if let Some(it) = parent.pat() {
                                    return is_match(it.syntax())
                                }
                            },
                            _ => ()
                        }
                }
            }
            false
        })
    }

    fn is_expr(node: &SyntaxNode, offset: TextSize) -> bool {
        let mut in_expr = true;
        // Temporary for T153426323
        let _pctx = stdx::panic_context::enter(format!("\nis_expr"));
        let ancestor_offset = algo::ancestors_at_offset(node, offset)
            .map(|n| {
                if n.kind() == SyntaxKind::TYPE_SIG {
                    in_expr = false;
                };
                n
            })
            .take_while(|n| n.kind() != SyntaxKind::SOURCE_FILE)
            .last()
            .and_then(|n| n.first_token())
            .map(|tok: SyntaxToken| tok.text_range().start())
            .unwrap_or_default();
        if !in_expr {
            return false;
        }
        // Temporary for T153426323
        let _pctx = stdx::panic_context::enter(format!("\nCtx::is_expr"));
        if let Some(mut tok) = node.token_at_offset(offset).left_biased() {
            if tok.text_range().start() < ancestor_offset {
                return false;
            }
            while let Some(prev) = tok.prev_token() {
                tok = prev;
                match tok.kind() {
                    SyntaxKind::ANON_DASH_GT => return true,
                    _ => (),
                }
            }
            false
        } else {
            false
        }
    }

    fn is_type(node: &SyntaxNode, offset: TextSize) -> bool {
        // Temporary for T153426323
        let _pctx = stdx::panic_context::enter(format!("\nis_type"));
        for n in algo::ancestors_at_offset(node, offset) {
            match_ast! {
                match n {
                    ast::Spec(_) => {
                        return true;
                    },
                    ast::TypeName(_) => {
                        return false;
                    },
                    ast::TypeAlias(_) => {
                        return true;
                    },
                    ast::Opaque(_) => {
                        return true;
                    },
                    ast::FieldType(_) => {
                        return true;
                    },
                    _ => ()
                }
            };
        }
        false
    }
    fn is_in_error(node: &SyntaxNode, offset: TextSize) -> bool {
        // Temporary for T153426323
        let _pctx = stdx::panic_context::enter(format!("\nis_in_error"));
        algo::ancestors_at_offset(node, offset).any(|n| n.kind() == SyntaxKind::ERROR)
    }
    fn previous_non_trivia_sibling_or_token(node: &SyntaxNode) -> Option<SyntaxElement> {
        let mut sot = node.prev_sibling_or_token();
        while let Some(NodeOrToken::Token(inner)) = sot {
            if !inner.kind().is_trivia() {
                return Some(inner.syntax_element());
            } else {
                sot = inner.prev_sibling_or_token();
            }
        }
        None
    }
}

/// Tests of internals, delete when autocomplete is full-featured T126163525
#[cfg(test)]
mod ctx_tests {
    use elp_ide_db::elp_base_db::fixture::WithFixture;
    use elp_ide_db::elp_base_db::FilePosition;
    use elp_ide_db::RootDatabase;
    use elp_syntax::AstNode;
    use hir::Semantic;

    use crate::Ctx;

    fn ctx(code: &str) -> Ctx {
        let (db, FilePosition { file_id, offset }) = RootDatabase::with_position(code);
        let sema = Semantic::new(&db);
        let parsed = sema.parse(file_id);
        let node = parsed.value.syntax();
        Ctx::new(node, offset)
    }

    #[test]
    fn expr_ctx() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            ~X.
        "#),
            Ctx::Expr
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            case 1 of.
                1 -> ~2
            end.
        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            fun(_) -> ~X end.
        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            try 1
            of
              1 -> X~
            catch
                _:_ -> ok
            catch
                _:_ -> ok
            end.
        "#),
            Ctx::Expr
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        main(_) ->
            #{(maps:from_list([~])) => 3}.
        "#),
            Ctx::Expr
        );
    }

    #[test]
    fn expr_ctx_2() {
        assert_eq!(
            ctx(r#"
        -module(completion).

        start() ->
            lists:~
            ok = preload_modules(),
            ok.
        "#),
            Ctx::Expr // Ctx::Other
        );
    }

    #[test]
    fn ctx_pattern() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        test(Y, X) ->
            ~Y = X.
        "#),
            Ctx::Other,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(X) ->
            case rand:uniform(1) of
                {X~} -> true
            end.
        "#),
            Ctx::Other,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(X) ->
            fun(X~) -> 1 end.
        "#),
            Ctx::Other,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            receive
                [X~] -> true
            end.
        "#),
            Ctx::Other,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            try [1]
            of
              [X~] -> true
            catch
                _:_ -> ok
            end.
        "#),
            Ctx::Other,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(X) ->
            if
                X~ -> ok
                true -> error
            end.

        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(X~) ->
            ok.
        "#),
            Ctx::Other,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(Y, X) ->
            try ok of
                X~ ->

        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(Y, X) ->
            try ok of
                ok -> ok
            catch
                X~ -> ok
        "#),
            Ctx::Other,
        );
    }

    #[test]
    // Known cases where error recovery for detecting context is inaccurate.
    // AST-based techniques may be more accurate, see D39766695 for details.
    fn ctx_pattern_error_recovery_wip() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        test(Y, X) ->
            try ok of
                X~ ->

        "#),
            // should be Ctx::Other
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test(Y, X) ->
            try ok of
                ok -> ok
            catch
                X~
        "#),
            // should be Ctx::Other
            Ctx::Expr,
        );
    }

    #[test]
    fn test_type_param_ctx() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        -type ty(s~) :: ok.
        "#),
            Ctx::Other
        );
    }

    #[test]
    fn test_export_ctx() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        -export([
            f~
        ])
        "#),
            Ctx::Export
        );
    }

    #[test]
    fn test_export_type_ctx() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        -export_type([
            t~
        ])
        "#),
            Ctx::ExportType
        );
    }

    #[test]
    fn test_type_ctx() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        -spec test() -> ~
        test() -> ok.
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -spec test() -> o~k
        test() -> ok.
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -spec test(o~) -> ok.
        test() -> ok.
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -record(foo, {field1, field2 :: X~}).
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -opaque test() :: ~.
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -type test() :: m~
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -spec test() -> ~ok.
        "#),
            Ctx::Type
        );
    }

    #[test]
    fn test_ctx_error_recovery() {
        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            ~
        "#),
            Ctx::Expr
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            X + ~
        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            X + ~.
        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            case rand:uniform(1) of
                1 -> ~X

        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            (erlang:term_to_binary(~

        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        test() ->
            (erlang:term_to_binary(~.

        "#),
            Ctx::Expr,
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -type ty() :: ~
        "#),
            Ctx::Other
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -type ty() :: l~.
        "#),
            Ctx::Type
        );

        assert_eq!(
            ctx(r#"
        -module(sample).
        -record(rec, {field = lists:map(fun(X) -> X + 1 end, [1, ~])}).
        "#),
            Ctx::Expr,
        );
    }
}
