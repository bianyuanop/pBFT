#!/bin/bash

f=$1
cnt=$f*3+1

for ((i=0; i<cnt; i++)); do
    gnome-terminal -- cargo run -- $i $f
done
