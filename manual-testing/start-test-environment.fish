#!/usr/bin/fish

set -x NC_USER tester
set -x NC_PASSWORD password
set -x NC_HOST localhost
set -x NC_PORT 8080
set -x DOCKER_NAME nc_demo
set -x NC_FOLDER test_folder

echo 'Starting Nextcloud docker'
docker run --rm --publish $NC_PORT:80 --detach --name $DOCKER_NAME nextcloud:latest
sleep 5;

echo "Creating root user $NC_USER with password $NC_PASSWORD"
bash create-root-user.sh || exit

echo "Syncing directory tree $NC_FOLDER to Nextcloud" 
bash sync-dir.sh $NC_FOLDER

set -x RUST_LOG nextcloud_tag_sync=trace,nextcloud_tag_sync::remote_fs::requests::common=debug

cd (status dirname)

echo "Starting bash shell for interactive testing"
bash --rcfile (echo 'source nc_tag.sh;' | psub )

echo "Stopping Nextcloud docker"
docker stop $DOCKER_NAME
