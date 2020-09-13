#!/usr/bin/env zsh

for i in {1..25}
do
    fuzz/text-gen.pl &
done
