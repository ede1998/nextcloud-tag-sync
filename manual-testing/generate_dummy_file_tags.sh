#!/bin/bash

source nc_tag.sh

# exactly the same tags
add_tag_to_file test_folder/foo/ignore.txt yellow
add_tag_to_file test_folder/foo/ignore.txt blue


# only remote tags
add_tag_to_remote_file test_folder/bar/baz/random.txt pink
add_tag_to_remote_file test_folder/dummy/err.pdf violet

# only local tags
add_tag_to_local_file test_folder/bar/baz/drat.pdf black
add_tag_to_local_file test_folder/dummy/please.jpg blue

# partially shared tags
add_tag_to_file test_folder/bar/ok.pdf mango
add_tag_to_local_file test_folder/bar/ok.pdf apple
add_tag_to_remote_file test_folder/bar/ok.pdf banana
