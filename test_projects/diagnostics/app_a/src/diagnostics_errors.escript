#!/usr/bin/env escript
%% -*- erlang -*-
%%! -smp enable -sname factorial -mnesia debug verbose
%% Sample escript taken from: http://erlang.org/doc/man/escript.html
main([String]) ->
  try
    N = list_to_integer(String),
    F = fac(N),
    io:format("factorial ~w = ~w\n", [N,F])
  catch
    _:_ ->
      usage()
  end;
main(_) ->
  usage().

usage() ->
  io:format("usage: factorial integer\n"),
  halt(1).

fac(0) -> 1;
fac(N) -> N * fac(N-1).

func tion_with_error() ->
  ok.

