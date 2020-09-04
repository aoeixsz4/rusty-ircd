#!/usr/bin/env zsh

for i in {1..20}
do
    fuzz/text-gen.pl &
done
