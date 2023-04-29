FOLDER_TO_UPLOAD=$(realpath "$1")

# delete existing files in test folder
curl --silent --user "$NC_USER:$NC_PASSWORD" 'http://'"$NC_HOST":"$NC_PORT"'/remote.php/dav/files/'"$NC_USER/$NC_FOLDER" --request DELETE > /dev/null || true

# create folders for test
find "$FOLDER_TO_UPLOAD" -type d | while read directory; do
directory=$(realpath --relative-to="$FOLDER_TO_UPLOAD" "$directory");
curl --user "$NC_USER:$NC_PASSWORD" 'http://'"$NC_HOST":"$NC_PORT"'/remote.php/dav/files/'"$NC_USER/$NC_FOLDER/$directory" --request MKCOL
done

# create files in folders
find "$FOLDER_TO_UPLOAD" -type f | while read file; do
file_without_test_root_dir=$(realpath --relative-to="$FOLDER_TO_UPLOAD" "$file");
curl --user "$NC_USER:$NC_PASSWORD" 'http://'"$NC_HOST":"$NC_PORT"'/remote.php/dav/files/'"$NC_USER/$NC_FOLDER/$file_without_test_root_dir" --request PUT --upload-file "$file"
done

# create config.toml
cat > config.toml << EOF
keep_side_on_conflict = "Both"
nextcloud_instance = "http://$NC_HOST:$NC_PORT"
user = "$NC_USER"
token = "$NC_PASSWORD"

[[prefixes]]
local = "$FOLDER_TO_UPLOAD"
remote = "/remote.php/dav/files/$NC_USER/$NC_FOLDER"
EOF

