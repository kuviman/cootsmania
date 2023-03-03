#!/usr/bin/env bash

./server/cootsmania --server localhost:1155 &
cootsmania=$!
./caddy/caddy run &
caddy=$!

wait -n $cootsmania $caddy
result=$?
kill $cootsmania $caddy &>/dev/null
exit $result