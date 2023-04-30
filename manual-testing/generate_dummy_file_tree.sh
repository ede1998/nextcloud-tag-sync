#!/bin/bash

mkdir -p test_folder/{bar/baz/,dummy/,foo}

echo "this is just a dummy file" > test_folder/foo/ignore.txt
echo "lorem ipsum" > test_folder/bar/baz/random.txt
touch test_folder/bar/baz/drat.pdf
touch test_folder/bar/ok.pdf
touch test_folder/dummy/err.pdf
touch test_folder/dummy/please.jpg
