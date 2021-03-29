#!/bin/bash
echo Running tests to update test count in README
TEST_COUNT=$(cargo test --workspace | sed -n "s/running \([[:digit:]]\+\) tests/\1/p" | awk '{sum = sum + $0}END{print sum}')
echo "{\"schemaVersion\": 1, \"label\": \"tests\", \"message\": \"$TEST_COUNT\", \"color\": \"blue\"}" > .build/test_count.json
