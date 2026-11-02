last=''; for a in "$@"; do last=$a; done
case "$1" in
ps)
  case "$last" in
    name=*) exit 0;;
    label=*)
      key=${last#label=}; key=${key%%=*}; value=${last#label=$key=}
      for f in "$S"/*.state; do
        [ -e "$f" ] || continue
        id=$(basename "$f" .state)
        case "$(cat "$S/$id.labels")" in
          *"\"$key\":\"$value\""*) echo "$id";;
        esac
      done;;
    *)
      for f in "$S"/*.state; do [ -e "$f" ] || continue; basename "$f" .state; done;;
  esac;;
inspect)
  shift 3
  printf '['; sep=''
  for id in "$@"; do
    [ -e "$S/$id.state" ] || { echo "No such container: $id" >&2; exit 1; }
    state=$(cat "$S/$id.state")
    polls=$(cat "$S/$id.polls" 2>/dev/null || echo 0)
    if [ "$polls" -gt 0 ]; then
      echo $((polls - 1)) > "$S/$id.polls"
      state='{"Status":"running","Health":{"Status":"starting"}}'
    elif [ "$state" = '{"Status":"running"}' ] && [ ! -e "$S/$id.ready" ]; then
      touch "$S/$id.ready"; echo "ready $(cat "$S/$id.name")" >> "$LOG"
    fi
    printf '%s{"Id":"%s","Name":"/%s","Image":"sha256:x","Config":{"Image":"img","Labels":{%s}},"State":%s}' \
      "$sep" "$id" "$(cat "$S/$id.name")" "$(cat "$S/$id.labels")" "$state"
    sep=','
  done
  echo ']';;
run)
  n=$(cat "$S/counter" 2>/dev/null || echo 0); n=$((n + 1)); echo "$n" > "$S/counter"
  id=$(printf '%064d' "$n")
  prev=''; labels=''; svc=''; name=''
  for a in "$@"; do
    case "$prev" in
      --name) name=$a;;
      --label) k=${a%%=*}; v=${a#*=}; labels="$labels${labels:+,}\"$k\":\"$v\""
        [ "$k" = riku.service ] && svc=$v;;
    esac
    prev=$a
  done
  echo "$name" > "$S/$id.name"; printf '%s' "$labels" > "$S/$id.labels"
  if [ -e "$S/exits-$svc" ]; then
    echo '{"Status":"exited","ExitCode":1}' > "$S/$id.state"
  else
    echo '{"Status":"running"}' > "$S/$id.state"
  fi
  [ -e "$S/slow-$svc" ] && echo 2 > "$S/$id.polls"
  echo "$id";;
stop) echo '{"Status":"exited","ExitCode":0}' > "$S/$last.state";;
wait) if [ -e "$S/release-exits" ]; then echo 1; else echo 0; fi;;
rm) rm -f "$S/$last".*;;
image) if [ -e "$S/image-id" ]; then cat "$S/image-id"; else echo sha256:x; fi;;
network)
  case "$2" in
  ls) for f in "$S"/net-*; do [ -e "$f" ] || continue; basename "$f" | cut -c5-; done;;
  create) touch "$S/net-$3";;
  rm) rm -f "$S/net-$3";;
  esac;;
volume)
  case "$2" in
  ls) for f in "$S"/vol-*; do [ -e "$f" ] || continue; basename "$f" | cut -c5-; done;;
  inspect)
    if [ -f "$S/vol-$last" ]; then
      cat "$S/vol-$last"
      echo
    else
      echo "no such volume" >&2
      exit 1
    fi;;
  esac;;
esac
