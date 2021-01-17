#!/bin/bash

BRANCH="$GITHUB_REF_SLUG"

if [[ -z "$BRANCH" ]]; then
	echo Branch var not set
	exit 1
fi

echo "Branch is $BRANCH"

if [[ "$BRANCH" = "develop" && -f .planning/active.md ]]; then
	echo "develop should not have an active.md"
	exit 1
fi
