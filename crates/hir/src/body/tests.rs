/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use elp_base_db::fixture::WithFixture;
use expect_test::expect;
use expect_test::Expect;

use crate::db::MinDefDatabase;
use crate::test_db::TestDB;
use crate::AnyAttribute;
use crate::FormIdx;
use crate::InFile;
use crate::SpecOrCallback;

#[track_caller]
fn check(ra_fixture: &str, expect: Expect) {
    let (db, file_id) = TestDB::with_single_file(ra_fixture);
    let form_list = db.file_form_list(file_id);
    let pretty = form_list
        .forms()
        .iter()
        .flat_map(|&form_idx| match form_idx {
            FormIdx::Function(function_id) => {
                let function = &form_list[function_id];
                let body = db.function_body(InFile::new(file_id, function_id));
                Some(body.print(&db, function))
            }
            FormIdx::TypeAlias(type_alias_id) => {
                let type_alias = &form_list[type_alias_id];
                let body = db.type_body(InFile::new(file_id, type_alias_id));
                Some(body.print(&db, type_alias))
            }
            FormIdx::Spec(spec_id) => {
                let spec = SpecOrCallback::Spec(form_list[spec_id].clone());
                let body = db.spec_body(InFile::new(file_id, spec_id));
                Some(body.print(&db, spec))
            }
            FormIdx::Callback(callback_id) => {
                let spec = SpecOrCallback::Callback(form_list[callback_id].clone());
                let body = db.callback_body(InFile::new(file_id, callback_id));
                Some(body.print(&db, spec))
            }
            FormIdx::Record(record_id) => {
                let body = db.record_body(InFile::new(file_id, record_id));
                Some(body.print(&db, &form_list, record_id))
            }
            FormIdx::Attribute(attribute_id) => {
                let attribute = AnyAttribute::Attribute(form_list[attribute_id].clone());
                let body = db.attribute_body(InFile::new(file_id, attribute_id));
                Some(body.print(&db, attribute))
            }
            FormIdx::CompileOption(attribute_id) => {
                let attribute = AnyAttribute::CompileOption(form_list[attribute_id].clone());
                let body = db.compile_body(InFile::new(file_id, attribute_id));
                Some(body.print(&db, attribute))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    expect.assert_eq(pretty.trim_start());
}

#[test]
fn simple() {
    check(
        r#"
foo(ok) -> ok.
"#,
        expect![[r#"
            foo('ok') ->
                'ok'.
        "#]],
    );
}

#[test]
fn char() {
    check(
        r#"
foo($a) -> $b.
"#,
        expect![[r#"
            foo($a) ->
                $b.
        "#]],
    );
}

#[test]
fn float() {
    check(
        r#"
foo(0.1) -> 1.2.
"#,
        expect![[r#"
            foo(0.1) ->
                1.2.
        "#]],
    );
}

#[test]
fn integer() {
    check(
        r#"
foo(42) -> 4_2.
"#,
        expect![[r#"
            foo(42) ->
                42.
        "#]],
    );
}

#[test]
fn string() {
    check(
        r#"
foo("") -> "abc\s\x61".
"#,
        expect![[r#"
            foo("") ->
                "abc a".
        "#]],
    );
}

#[test]
fn concat() {
    check(
        r#"
foo("" "") -> "a" "b" "c" "\141".
"#,
        expect![[r#"
            foo("") ->
                "abca".
        "#]],
    );
}

#[test]
fn var() {
    check(
        r#"
foo(Foo) -> Foo.
"#,
        expect![[r#"
            foo(Foo) ->
                Foo.
        "#]],
    );
}

#[test]
fn tuple() {
    check(
        r#"
foo({a, b}) -> {1, 2, 3}.
"#,
        expect![[r#"
            foo({
                'a',
                'b'
            }) ->
                {
                    1,
                    2,
                    3
                }.
        "#]],
    );
}

#[test]
fn list() {
    check(
        r#"
foo([a, b]) -> [1, 2, 3].
"#,
        expect![[r#"
            foo([
                'a',
                'b'
            ]) ->
                [
                    1,
                    2,
                    3
                ].
        "#]],
    );

    check(
        r#"
foo([a | b]) -> [1, 2 | 3].
"#,
        expect![[r#"
            foo([
                'a'
                | 'b'
            ]) ->
                [
                    1,
                    2
                    | 3
                ].
        "#]],
    );
}

#[test]
fn r#match() {
    check(
        r#"
foo(A = B) -> A = B.
"#,
        expect![[r#"
            foo(A = B) ->
                A = B.
        "#]],
    );
}

#[test]
fn unary_op() {
    check(
        r#"
foo(+ A, -B) -> bnot A, not C.
"#,
        expect![[r#"
            foo((+ A), (- B)) ->
                (bnot A),
                (not C).
        "#]],
    );
}

#[test]
fn binary_op() {
    check(
        r#"
foo(A ++ B, C + D) -> E andalso F, G ! H.
"#,
        expect![[r#"
            foo((A ++ B), (C + D)) ->
                (E andalso F),
                (G ! H).
        "#]],
    );
}

#[test]
fn map() {
    check(
        r#"
foo(#{1 + 2 := 3 + 4}) -> #{a => b}.
"#,
        expect![[r##"
            foo(#{
                (1 + 2) := (3 + 4)
            }) ->
                #{
                    'a' => 'b'
                }.
        "##]],
    );
}

#[test]
fn map_update() {
    check(
        r#"
foo() -> #{a => b}#{a := b, c => d}.
"#,
        expect![[r##"
            foo() ->
                #{
                    'a' => 'b'
                }#{
                    'a' := 'b',
                    'c' => 'd'
                }.
        "##]],
    );
}

#[test]
fn record_index() {
    check(
        r#"
foo(#record.field) -> #record.field.
"#,
        expect![[r##"
            foo(#record.field) ->
                #record.field.
        "##]],
    );
}

#[test]
fn record() {
    check(
        r#"
foo1(#record{field = 1}) -> #record{field = A + B}.
foo2(#record{field}) -> #record{field = }.
"#,
        expect![[r##"
            foo1(#record{
                field = 1
            }) ->
                #record{
                    field = (A + B)
                }.

            foo2(#record{
                field = [missing]
            }) ->
                #record{
                    field = [missing]
                }.
        "##]],
    );
}

#[test]
fn record_update() {
    check(
        r#"
foo1() -> Expr#record{field = undefined}.
foo2() -> Expr#record{field = ok, missing = }.
"#,
        expect![[r##"
            foo1() ->
                Expr#record{
                    field = 'undefined'
                }.

            foo2() ->
                Expr#record{
                    field = 'ok',
                    missing = [missing]
                }.
        "##]],
    );
}

#[test]
fn record_field() {
    check(
        r#"
foo() -> Expr#record.field.
"#,
        expect![[r##"
            foo() ->
                Expr#record.field.
        "##]],
    );
}

#[test]
fn binary() {
    check(
        r#"
foo(<<Size, Data:Size/binary>>) -> <<+1/integer-little-unit:8>>.
"#,
        expect![[r#"
            foo(<<
                Size,
                Data:Size/binary
            >>) ->
                <<
                    (+ 1)/integer-little-unit:8
                >>.
        "#]],
    );
}

#[test]
fn catch() {
    check(
        r#"
foo() -> catch 1 + 2.
"#,
        expect![[r#"
            foo() ->
                (catch (1 + 2)).
        "#]],
    );
}

#[test]
fn begin_block() {
    check(
        r#"
foo() -> begin 1, 2 end.
"#,
        expect![[r#"
            foo() ->
                begin
                    1,
                    2
                end.
        "#]],
    );
}

#[test]
fn case() {
    check(
        r#"
foo() ->
    case 1 + 2 of
        X when X andalso true; X <= 100, X >= 5 -> ok;
        _ -> error
    end.
"#,
        expect![[r#"
            foo() ->
                case (1 + 2) of
                    X when
                        (X andalso 'true');
                        (X < 100),
                        (X >= 5)
                    ->
                        'ok';
                    _ ->
                        'error'
                end.
        "#]],
    );
}

#[test]
fn receive() {
    check(
        r#"
foo() ->
    receive
        ok when true -> ok;
        _ -> error
    after Timeout -> timeout
    end.
"#,
        expect![[r#"
            foo() ->
                receive
                    'ok' when
                        'true'
                    ->
                        'ok';
                    _ ->
                        'error'
                after Timeout ->
                    'timeout'
                end.
        "#]],
    );
}

#[test]
fn call() {
    check(
        r#"
foo() ->
    foo(),
    foo:bar(A).
"#,
        expect![[r#"
            foo() ->
                'foo'(),
                'foo':'bar'(
                    A
                ).
        "#]],
    );
}

#[test]
fn capture_fun() {
    check(
        r#"
foo() ->
    fun foo/1,
    fun mod:foo/1,
    fun Mod:Foo/Arity.
"#,
        expect![[r#"
            foo() ->
                fun 'foo'/1,
                fun 'mod':'foo'/1,
                fun Mod:Foo/Arity.
        "#]],
    );
}

#[test]
fn if_expr() {
    check(
        r#"
foo() ->
    if is_atom(X) -> ok;
       true -> error
    end.
"#,
        expect![[r#"
            foo() ->
                if
                    'is_atom'(
                        X
                    ) ->
                        'ok';
                    'true' ->
                        'error'
                end.
        "#]],
    );
}

#[test]
fn try_expr() {
    check(
        r#"
foo() ->
    try 1, 2 of
        _ -> ok
    catch
        Pat when true -> ok;
        error:undef:Stack -> Stack
    after
        ok
    end.
"#,
        expect![[r#"
            foo() ->
                try
                    1,
                    2
                of
                    _ ->
                        'ok'
                catch
                    Pat when
                        'true'
                    ->
                        'ok';
                    'error':'undef':Stack ->
                        Stack
                after
                    'ok'
                end.
        "#]],
    );
}

#[test]
fn comprehensions() {
    check(
        r#"
foo() ->
    [X || X <- List, X >= 5],
    << Byte || <<Byte>> <= Bytes, Byte >= 5>>,
    #{KK => VV || KK := VV <- Map}.
"#,
        expect![[r#"
            foo() ->
                [
                    X
                ||
                    X <- List,
                    (X >= 5)
                ],
                <<
                    Byte
                ||
                    <<
                        Byte
                    >> <= Bytes,
                    (Byte >= 5)
                >>,
                #{
                    KK => VV
                ||
                    KK := VV <- Map
                }.
        "#]],
    );
}

#[test]
fn fun() {
    check(
        r#"
foo() ->
    fun (ok) -> ok;
        (error) -> error
    end,
    fun Named() -> Named() end.
"#,
        expect![[r#"
            foo() ->
                fun
                    ('ok') ->
                        'ok';
                    ('error') ->
                        'error'
                end,
                fun
                    Named() ->
                        Named()
                end.
        "#]],
    );
}

#[test]
fn parens() {
    check(
        r#"
foo((ok), ()) ->
    (ok),
    ().
"#,
        expect![[r#"
            foo('ok', [missing]) ->
                'ok',
                [missing].
        "#]],
    );
}

#[test]
fn invalid_ann_type() {
    check(
        r#"
foo(A :: {}) -> A :: {}.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_dotdotdot() {
    check(
        r#"
foo(...) -> ....
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_call() {
    check(
        r#"
foo(bar()) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_catch() {
    check(
        r#"
foo(catch 1) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_map_update() {
    check(
        r#"
foo(X#{}) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_pipe() {
    check(
        r#"
foo(X | Y) -> X | Y.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_range() {
    check(
        r#"
foo(X..Y) -> X..Y.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_record_field() {
    check(
        r#"
foo(X#foo.bar) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_record_update() {
    check(
        r#"
foo(X#foo{}) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_remote() {
    check(
        r#"
foo(a:b) -> a:b.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_fun() {
    check(
        r#"
foo(fun() -> ok end) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_comprehension() {
    check(
        r#"
foo(<<Byte || Byte <- List>>, [Byte || Byte <- List]]) -> ok.
"#,
        expect![[r#"
            foo([missing], [missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_block() {
    check(
        r#"
foo(begin foo end) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_case() {
    check(
        r#"
foo(case X of _ -> ok end) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_capture() {
    check(
        r#"
foo(fun erlang:self/0, fun foo/2) -> ok.
"#,
        expect![[r#"
            foo([missing], [missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_fun_type() {
    check(
        r#"
foo(fun()) -> fun().
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_if() {
    check(
        r#"
foo(if true -> ok end) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_macro_string() {
    check(
        r#"
foo(??X) -> ??X.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_receive() {
    check(
        r#"
foo(receive _ -> ok after X -> timeout end) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_try() {
    check(
        r#"
foo(try 1 of _ -> ok catch _ -> error end) -> ok.
"#,
        expect![[r#"
            foo([missing]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn invalid_concat() {
    check(
        r#"
foo("a" B "c") -> "a" B "c".
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn invalid_macro_case_clause() {
    check(
        r#"
foo() ->
    case X of
        ?MACRO();
        ok -> ok
    end.
"#,
        expect![[r#"
            foo() ->
                case X of
                    'ok' ->
                        'ok'
                end.
        "#]],
    );
}

#[test]
fn macro_exprs() {
    check(
        r#"
foo(?MACRO()) -> ?MACRO().
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn simple_type() {
    check(
        r#"
-type foo() :: ok.
"#,
        expect![[r#"
            -type foo() :: 'ok'.
        "#]],
    );
}

#[test]
fn simple_opaque() {
    check(
        r#"
-opaque foo() :: ok.
"#,
        expect![[r#"
            -opaque foo() :: 'ok'.
        "#]],
    );
}

#[test]
fn unary_op_type() {
    check(
        r#"
-type foo() :: -1.
"#,
        expect![[r#"
            -type foo() :: (- 1).
        "#]],
    );
}

#[test]
fn binary_op_type() {
    check(
        r#"
-type foo() :: 1 + 1.
"#,
        expect![[r#"
            -type foo() :: (1 + 1).
        "#]],
    );
}

#[test]
fn ann_type() {
    check(
        r#"
-type foo() :: A :: any().
"#,
        expect![[r#"
            -type foo() :: (A  :: 'any'()).
        "#]],
    );
}

#[test]
fn list_type() {
    check(
        r#"
-type foo() :: [foo].
-type bar() :: [bar, ...].
"#,
        expect![[r#"
            -type foo() :: ['foo'].

            -type bar() :: ['bar', ...].
        "#]],
    );
}

#[test]
fn tuple_type() {
    check(
        r#"
-type foo() :: {a, b, c}.
"#,
        expect![[r#"
            -type foo() :: {
                'a',
                'b',
                'c'
            }.
        "#]],
    );
}

#[test]
fn range_type() {
    check(
        r#"
-type foo() :: 1..100.
"#,
        expect![[r#"
            -type foo() :: (1..100).
        "#]],
    );
}

#[test]
fn map_type() {
    check(
        r#"
-type foo() :: #{a => b, c := d}.
"#,
        expect![[r##"
            -type foo() :: #{
                'a' => 'b',
                'c' := 'd'
            }.
        "##]],
    );
}

#[test]
fn fun_type() {
    check(
        r#"
-type foo1() :: fun().
-type foo2() :: fun(() -> ok).
-type foo3() :: fun((a, b) -> ok).
-type foo4() :: fun((...) -> ok).
"#,
        expect![[r#"
            -type foo1() :: fun().

            -type foo2() :: fun(() -> 'ok').

            -type foo3() :: fun(('a', 'b') -> 'ok').

            -type foo4() :: fun((...) -> 'ok').
        "#]],
    );
}

#[test]
fn union_type() {
    check(
        r#"
-type foo1() :: a | b.
-type foo2() :: a | b | c.
-type foo3() :: (a | b) | c.
"#,
        expect![[r#"
            -type foo1() :: (
                'a' |
                'b'
            ).

            -type foo2() :: (
                'a' |
                'b' |
                'c'
            ).

            -type foo3() :: (
                (
                    'a' |
                    'b'
                ) |
                'c'
            ).
        "#]],
    );
}

#[test]
fn var_type() {
    check(
        r#"
-type foo(A) :: A.
"#,
        expect![[r#"
            -type foo(A) :: A.
        "#]],
    );
}

#[test]
fn call_type() {
    check(
        r#"
-type local(A) :: local(A | integer()).
-type remote(A) :: module:remote(A | integer()).
"#,
        expect![[r#"
            -type local(A) :: 'local'(
                (
                    A |
                    'integer'()
                )
            ).

            -type remote(A) :: 'module':'remote'(
                (
                    A |
                    'integer'()
                )
            ).
        "#]],
    );
}

#[test]
fn record_type() {
    check(
        r#"
-type foo1() :: #record{}.
-type foo2(B) :: #record{a :: integer(), b :: B}.
-type foo3() :: #record{a ::}.
"#,
        expect![[r##"
            -type foo1() :: #record{}.

            -type foo2(B) :: #record{
                a :: 'integer'(),
                b :: B
            }.

            -type foo3() :: #record{
                a :: [missing]
            }.
        "##]],
    );
}

#[test]
fn invalid_type() {
    check(
        r#"
-type foo() :: catch 1.
"#,
        expect![[r#"
            -type foo() :: [missing].
        "#]],
    );
}

#[test]
fn simple_spec() {
    check(
        r#"
-spec foo() -> ok.
"#,
        expect![[r#"
            -spec foo
                () -> 'ok'.
        "#]],
    );
}

#[test]
fn simple_callback() {
    check(
        r#"
-callback foo() -> ok.
"#,
        expect![[r#"
            -callback foo
                () -> 'ok'.
        "#]],
    );
}

#[test]
fn multi_sig_spec() {
    check(
        r#"
-spec foo(atom()) -> atom();
         (integer()) -> integer().
"#,
        expect![[r#"
            -spec foo
                ('atom'()) -> 'atom'();
                ('integer'()) -> 'integer'().
        "#]],
    );
}

#[test]
fn ann_var_spec() {
    check(
        r#"
-spec foo(A :: any()) -> ok.
"#,
        expect![[r#"
            -spec foo
                ((A  :: 'any'())) -> 'ok'.
        "#]],
    );
}

#[test]
fn guarded_spec() {
    check(
        r#"
-spec foo(A) -> A
    when A :: any().
"#,
        expect![[r#"
            -spec foo
                (A) -> A
                    when A :: 'any'().
        "#]],
    );
}

#[test]
fn record_definition() {
    check(
        r#"
-record(foo, {}).
-record(foo, {field}).
-record(foo, {field = value}).
-record(foo, {field :: type}).
-record(foo, {field = value :: type}).
"#,
        expect![[r#"
            -record(foo, {
            }).

            -record(foo, {
                field
            }).

            -record(foo, {
                field = 'value'
            }).

            -record(foo, {
                field :: 'type'
            }).

            -record(foo, {
                field = 'value' :: 'type'
            }).
        "#]],
    );
}

#[test]
fn simple_term() {
    check(
        r#"
-foo(ok).
-missing_value().
"#,
        expect![[r#"
            -foo('ok').

            -missing_value([missing]).
        "#]],
    );
}

#[test]
fn tuple_term() {
    check(
        r#"
-foo({1, 2, ok, "abc"}).
"#,
        expect![[r#"
            -foo({
                1,
                2,
                'ok',
                "abc"
            }).
        "#]],
    );
}

#[test]
fn list_term() {
    check(
        r#"
-foo([]).
-bar([1, 2]).
-baz([1 | 2]).
"#,
        expect![[r#"
            -foo([]).

            -bar([
                1,
                2
            ]).

            -baz([
                1
                | 2
            ]).
        "#]],
    );
}

#[test]
fn map_term() {
    check(
        r#"
-foo(#{1 => 2}).
"#,
        expect![[r##"
            -foo(#{
                1 => 2
            }).
        "##]],
    );
}

#[test]
fn fun_term() {
    check(
        r#"
-foo(fun erlang:is_integer/1).
"#,
        expect![[r#"
            -foo(fun erlang:is_integer/1).
        "#]],
    );
}

#[test]
fn unary_op_term() {
    check(
        r#"
-foo(-(1)).
-foo(-(1.5)).
-foo(-$a).
-foo(+1).
-foo(+1.5).
-foo(+$a).
-foo(-atom).
"#,
        expect![[r#"
            -foo(-1).

            -foo(-1.5).

            -foo(-97).

            -foo(1).

            -foo(1.5).

            -foo($a).

            -foo([missing]).
        "#]],
    );
}

#[test]
fn binary_op_term() {
    check(
        r#"
-foo(foo/1).
-compile({inline, [foo/1]}).
-compile({a/a, 1/1}).
"#,
        expect![[r#"
            -foo({
                'foo',
                1
            }).

            -compile({
                'inline',
                [
                    {
                        'foo',
                        1
                    }
                ]
            }).

            -compile({
                [missing],
                [missing]
            }).
        "#]],
    );
}

#[test]
fn binary_term() {
    check(
        r#"
-foo(<<"abc">>).
-bar(<<"abc", "def">>).
-baz(<<$a, $b, $c>>).
-foobar(<<1, 2, 3, -1>>).
"#,
        expect![[r#"
            -foo(<<"abc"/utf8>>).

            -bar(<<"abcdef"/utf8>>).

            -baz(<<"abc"/utf8>>).

            -foobar(<<1, 2, 3, 255>>).
        "#]],
    );
}

#[test]
fn expand_macro_function_clause() {
    check(
        r#"
-define(CLAUSE, foo(_) -> ok).

foo(1) -> 1;
?CLAUSE.
"#,
        expect![[r#"
            foo(1) ->
                1;
            foo(_) ->
                'ok'.
        "#]],
    );
}

#[test]
fn expand_macro_expr() {
    check(
        r#"
-define(EXPR, 1 + 2).

foo() -> ?EXPR.
"#,
        expect![[r#"
            foo() ->
                (1 + 2).
        "#]],
    );
}

#[test]
fn expand_macro_var_in_expr() {
    check(
        r#"
-define(EXPR(X), 1 + X).

foo() -> ?EXPR(2).
"#,
        expect![[r#"
            foo() ->
                (1 + 2).
        "#]],
    );
}

#[test]
fn expand_macro_function() {
    check(
        r#"
-define(NAME, name).

foo() -> ?NAME(2).
"#,
        expect![[r#"
            foo() ->
                'name'(
                    2
                ).
        "#]],
    );
}

#[test]
fn expand_macro_remote_function() {
    check(
        r#"
-define(NAME, module:name).

foo() -> ?NAME(2).
"#,
        expect![[r#"
            foo() ->
                'module':'name'(
                    2
                ).
        "#]],
    );
}

#[test]
fn expand_macro_pat() {
    check(
        r#"
-define(PAT, [_]).

foo(?PAT) -> ok.
"#,
        expect![[r#"
            foo([
                _
            ]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn expand_macro_var_in_pat() {
    check(
        r#"
-define(PAT(X), [X]).

foo(?PAT(_)) -> ok.
"#,
        expect![[r#"
            foo([
                _
            ]) ->
                'ok'.
        "#]],
    );
}

#[test]
fn expand_macro_type() {
    check(
        r#"
-define(TY, a | b).

-type foo() :: ?TY.
"#,
        expect![[r#"
            -type foo() :: (
                'a' |
                'b'
            ).
        "#]],
    );
}

#[test]
fn expand_macro_type_call() {
    check(
        r#"
-define(NAME, name).

-type foo() :: ?NAME().
"#,
        expect![[r#"
            -type foo() :: 'name'().
        "#]],
    );
}

#[test]
fn expand_macro_remote_type() {
    check(
        r#"
-define(NAME, module:name).

-type foo() :: ?NAME().
"#,
        expect![[r#"
            -type foo() :: 'module':'name'().
        "#]],
    );
}

#[test]
fn expand_macro_var_in_type() {
    check(
        r#"
-define(TY(X), a | X).

-type foo() :: ?TY(b).
"#,
        expect![[r#"
            -type foo() :: (
                'a' |
                'b'
            ).
        "#]],
    );
}

#[test]
fn expand_macro_term() {
    check(
        r#"
-define(TERM, [0, 1]).

-foo(?TERM).
"#,
        expect![[r#"
            -foo([
                0,
                1
            ]).
        "#]],
    );
}

#[test]
fn expand_macro_var_in_term() {
    check(
        r#"
-define(TERM(X), [0, X]).

-foo(?TERM(1)).
"#,
        expect![[r#"
            -foo([
                0,
                1
            ]).
        "#]],
    );
}

#[test]
fn expand_macro_cr_clause() {
    check(
        r#"
-define(CLAUSE(Pat, Expr), Pat -> Expr).

foo() ->
    case bar() of
        ?CLAUSE(ok, ok);
        ?CLAUSE(_, error)
    end.
"#,
        expect![[r#"
            foo() ->
                case 'bar'() of
                    'ok' ->
                        'ok';
                    _ ->
                        'error'
                end.
        "#]],
    );
}

#[test]
fn expand_macro_arity() {
    check(
        r#"
-define(ARITY(X), X).

foo() ->
    fun local/?ARITY(1),
    fun remote:function/?ARITY(2).
"#,
        expect![[r#"
            foo() ->
                fun 'local'/1,
                fun 'remote':'function'/2.
        "#]],
    );
}

#[test]
fn expand_macro_name() {
    check(
        r#"
-define(NAME, name).

foo() ->
    #?NAME{?NAME = ?NAME}.
"#,
        expect![[r#"
            foo() ->
                #name{
                    name = 'name'
                }.
        "#]],
    );
}

#[test]
fn expand_nested_macro() {
    check(
        r#"
-define(M1(A, B), {m1, ?M2(A, ?M3(B))}).
-define(M2(B, A), {m2, B, ?M3(A)}).
-define(M3(A), {m3, A}).

foo() ->
    ?M1(1, 2),
    ?M1(A, B).
"#,
        expect![[r#"
            foo() ->
                {
                    'm1',
                    {
                        'm2',
                        1,
                        {
                            'm3',
                            {
                                'm3',
                                2
                            }
                        }
                    }
                },
                {
                    'm1',
                    {
                        'm2',
                        A,
                        {
                            'm3',
                            {
                                'm3',
                                B
                            }
                        }
                    }
                }.
        "#]],
    );
}

#[test]
fn expand_built_in_function_name() {
    check(
        r#"
foo(?FUNCTION_NAME) -> ?FUNCTION_NAME.
"#,
        expect![[r#"
            foo('foo') ->
                'foo'.
        "#]],
    );

    check(
        r#"
foo() -> ?FUNCTION_NAME().
"#,
        expect![[r#"
            foo() ->
                'foo'().
        "#]],
    );
}

#[test]
fn expand_built_in_function_arity() {
    check(
        r#"
foo(?FUNCTION_ARITY) -> ?FUNCTION_ARITY.
"#,
        expect![[r#"
            foo(1) ->
                1.
        "#]],
    );
}

#[test]
fn expand_built_in_line() {
    check(
        r#"
-type foo() :: ?LINE.

foo(?LINE) -> ?LINE.
"#,
        expect![[r#"
            -type foo() :: 0.

            foo(0) ->
                0.
        "#]],
    );
}

#[test]
fn expand_built_in_machine() {
    check(
        r#"
-type foo() :: ?MACHINE.

foo(?MACHINE) -> ?MACHINE.
"#,
        expect![[r#"
            -type foo() :: 'ELP'.

            foo('ELP') ->
                'ELP'.
        "#]],
    );
}

#[test]
fn expand_built_in_otp_release() {
    check(
        r#"
-type foo() :: ?OTP_RELEASE.

foo(?OTP_RELEASE) -> ?OTP_RELEASE.
"#,
        expect![[r#"
            -type foo() :: 2000.

            foo(2000) ->
                2000.
        "#]],
    );
}

#[test]
fn expand_built_in_module_no_attribute() {
    check(
        r#"
-type foo() :: ?MODULE.

foo(?MODULE) -> ?MODULE.
"#,
        expect![[r#"
            -type foo() :: [missing].

            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn expand_built_in_module() {
    check(
        r#"
-module(foobar).

-type foo() :: ?MODULE.

foo(?MODULE) -> ?MODULE.
"#,
        expect![[r#"
            -type foo() :: 'foobar'.

            foo('foobar') ->
                'foobar'.
        "#]],
    );
}

#[test]
fn expand_built_in_module_string() {
    check(
        r#"
-module(foobar).

-type foo() :: ?MODULE_STRING.

foo(?MODULE_STRING) -> ?MODULE_STRING.
"#,
        expect![[r#"
            -type foo() :: "foobar".

            foo("foobar") ->
                "foobar".
        "#]],
    );
}

#[test]
fn expand_built_in_file() {
    check(
        r#"
-module(foobar).

-type foo() :: ?FILE.

foo(?FILE) -> ?FILE.
"#,
        expect![[r#"
            -type foo() :: "foobar.erl".

            foo("foobar.erl") ->
                "foobar.erl".
        "#]],
    );
}

#[test]
fn expand_recursive_macro() {
    check(
        r#"
-define(FOO, ?FOO).

foo(?FOO) -> ?FOO.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn expand_mutually_recursive_macro() {
    check(
        r#"
-define(FOO, ?BAR).
-define(BAR, ?FOO).

foo(?FOO) -> ?BAR.
"#,
        expect![[r#"
            foo([missing]) ->
                [missing].
        "#]],
    );
}

#[test]
fn expand_aparently_argument_recursive_macro() {
    check(
        r#"
-define(FOO(X), X).

foo(?FOO(?FOO(1))) -> ?FOO(?FOO(1)).
"#,
        expect![[r#"
            foo(1) ->
                1.
        "#]],
    );

    check(
        r#"
-define(FOO(X), ?BAR(X)).
-define(BAR(X), X).

foo(?FOO(?BAR(?FOO(?BAR(1))))) -> ?FOO(?BAR(?FOO(?BAR(1)))).
"#,
        expect![[r#"
            foo(1) ->
                1.
        "#]],
    );
}

#[test]
fn maybe_simple() {
    check(
        r#"
foo() ->
maybe
    {ok, A} ?= a(),
    true = A >= 0,
    {ok, B} ?= b(),
    A + B
end."#,
        expect![[r#"
        foo() ->
            maybe
                {
                    'ok',
                    A
                } ?= 'a'(),
                'true' = (A >= 0),
                {
                    'ok',
                    B
                } ?= 'b'(),
                (A + B)
            end.
        "#]],
    );
}

#[test]
fn maybe_else() {
    check(
        r#"
foo() ->
maybe
    {ok, A} ?= a(),
    true = A >= 0,
    A
else
    error -> error;
    Other when Other == 0 -> error
end."#,
        expect![[r#"
        foo() ->
            maybe
                {
                    'ok',
                    A
                } ?= 'a'(),
                'true' = (A >= 0),
                A
            else
                'error' ->
                    'error';
                Other when
                    (Other == 0)
                ->
                    'error'
            end.
        "#]],
    );
}
