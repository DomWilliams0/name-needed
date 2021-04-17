#!/bin/bash

BRANCH="$1"

if [[ -z "$BRANCH" ]]; then
	echo Branch not passed
	exit 1
fi

echo "Branch is $BRANCH"

if [[ "$BRANCH" = "develop" && -f .planning/active.md ]]; then
	echo "develop should not have an active.md"
	exit 1
fi
