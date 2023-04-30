#!/bin/bash

# Slightly adapted from https://github.com/68b32/nc_tag

_HOST="$NC_HOST:$NC_PORT"
_USERNAME="$NC_USER"
_PASSWORD="$NC_PASSWORD"
HTTP="http"

get_fileprop_by_path() {
	local path="$1"
	local prop="$2"
	curl -u $_USERNAME:$_PASSWORD "$HTTP://$_HOST/remote.php/dav/files/$_USERNAME/$path" -X PROPFIND --data '<?xml version="1.0" encoding="UTF-8"?>
	 <d:propfind xmlns:d="DAV:">
	   <d:prop xmlns:oc="http://owncloud.org/ns">
	     <oc:'"$prop"' />
	   </d:prop>
	 </d:propfind>' 2> /dev/null | xmlstarlet sel -t -v "//oc:$prop"
}



get_tags_from_server() {
	curl -s -u $_USERNAME:$_PASSWORD "$HTTP://$_HOST/remote.php/dav/systemtags"  -X PROPFIND --data '<?xml version="1.0" encoding="utf-8" ?>
	<a:propfind xmlns:a="DAV:" xmlns:oc="http://owncloud.org/ns">
	  <a:prop>
	    <oc:display-name/>
	    <oc:user-visible/>
	    <oc:user-assignable/>
	    <oc:id/>
	  </a:prop>
	</a:propfind>' | xmllint --format - | xmlstarlet sel -t -v "//oc:display-name | //oc:id" | grep -Pv '^$' | xargs -n2 -d'\n'
}

reload_tags() {
	_TAGS="`get_tags_from_server`"
}

validate_tagname() {
	local tag="$1"
	echo $tag | grep -P '[^0-9a-zA-Z\-]' &> /dev/null && echo "INVALID TAGNAME $tag" && return 1
}

tag_exists() {
	local needle="$1"
	validate_tagname "$tag"
	echo $_TAGS | grep -P "(^|\s)$needle [0-9]+" &> /dev/null
	return $?
}

get_id_for_tag() {
	local tag="$1"
	tag_exists "$tag" || return 1
	validate_tagname "$tag"
	echo $_TAGS | grep -Po "(^|\s)$tag [0-9]+" | awk '{print $2}'
	return $?
}

get_tag_for_id() {
	local tagid="$1"
	echo $_TAGS | grep -Po "(^|\s)[a-zA-Z0-9\-]+ $tagid(\s|$)" | awk '{print $1}'
}


get_tags_from_file() {
	local path="$1"
	fileid="`get_fileprop_by_path \"$path\" fileid`"
	echo $fileid | grep -P '[^0-9]' &> /dev/null && return 1
	[ -z "$fileid" ] && return 2

	curl -s -u $_USERNAME:$_PASSWORD "$HTTP://$_HOST/remote.php/dav/systemtags-relations/files/$fileid" -X PROPFIND --data '<?xml version="1.0" encoding="utf-8" ?>
	<a:propfind xmlns:a="DAV:" xmlns:oc="http://owncloud.org/ns">
	  <a:prop>
	    <oc:display-name/>
	    <oc:user-visible/>
	    <oc:user-assignable/>
	    <oc:id/>
	  </a:prop>
	</a:propfind>' | xmlstarlet sel -t -v "//oc:display-name" | grep -vP '^$'
}

file_has_tag() {
	local path="$1"
	local tag="$2"
	validate_tagname "$tag"
	get_tags_from_file "$path" | grep -P "^$tag$" && return 0
	return 1
}

add_tag_to_remote_file() {
	local path="$1"
	local tag="$2"
	validate_tagname "$tag"
	file_has_tag "$path" "$tag" && return 1

	fileid="`get_fileprop_by_path \"$path\" fileid`"
	echo $fileid | grep -P '[^0-9]' &> /dev/null && return 1
	[ -z "$fileid" ] && return 2

	tagid="`get_id_for_tag \"$tag\"`"
	echo $tagid | grep -P '[^0-9]' &> /dev/null && return 1
	if [ -z "$tagid" ]; then
	    add_tag_to_server "$tag"
		reload_tags
		tagid="`get_id_for_tag \"$tag\"`"
		echo $tagid | grep -P '[^0-9]' &> /dev/null && return 1
		[ -z "$tagid" ] && return 2
	fi

	curl -s -u $_USERNAME:$_PASSWORD "$HTTP://$_HOST/remote.php/dav/systemtags-relations/files/$fileid/$tagid" -X PUT
	return $?
}

add_tag_to_server() {
	local tag="$1"
	validate_tagname "$tag"
	tag_exists "$tag" && return 1
	curl -s -u $_USERNAME:$_PASSWORD "$HTTP://$_HOST/remote.php/dav/systemtags/" -X POST -H 'Content-Type: application/json' --data "{\"userVisible\":true,\"userAssignable\":true,\"canAssign\":true,\"name\":\"$tag\"}"
	return $?
}

add_tag_to_local_file() {
	local path="$1"
	local tag="$2"

	[ -f "$path" ] || return 1

    # read existing tags
	tags=()
	readarray -t -d , tags  <<< $( getfattr "$path" --name user.xdg.tags --only-values 2> /dev/null )
	
	# remove single item only contains '\n' -> no tags yet
	if [ "${#tags[@]}" -eq 1 ]; then
	  if [[ ! "${tags[0]}" = *[![:space:]]* ]]; then
	    tags=()
	  fi
	fi

	# filter duplicates
	(printf '%s\0' "${tags[@]}" | grep --fixed-strings --null-data --line-regexp --quiet "$tag") || tags+=($tag)

    # format as comma separated string
	stags=""
	for (( i=0; i<"${#tags[@]}"; i++ )); do
	  # strip trailing newlines
	  tag=$(sed -e 's/[[:space:]]*$//' <<< "${tags[$i]}" )

	  if [ "$i" -ne 0 ]; then
	    stags="$stags,${tag}";
	  else
	    stags="${tag}";
	  fi
	done

	# replace tags
	setfattr "$path" --value "$stags" --name "user.xdg.tags"
}

add_tag_to_file() {
	local path="$1"
	local tag="$2"
	add_tag_to_remote_file "$path" "$tag"
	add_tag_to_local_file "$path" "$tag"
}

list_local_tags() {
	getfattr --name user.xdg.tags test_folder/ --recursive 2> /dev/null
}