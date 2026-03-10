#!/bin/sh

if [ "$#" -eq 2 ]; then
	dot -Tpng $1 -o $2
else
	echo "Usage: $0 [input] [output]" >&2
	exit 2
fi
