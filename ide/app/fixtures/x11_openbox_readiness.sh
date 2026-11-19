#!/usr/bin/env bash
# Sourced only by the disposable installed X11 acceptance harnesses.
# A live process and a WM_CHECK property alone do not establish activation.
wait_for_openbox_ewmh() {
  local pid=$1 attempts=${2:-80} i root check supported owner id previous=''
  local pattern='^_NET_SUPPORTING_WM_CHECK\(WINDOW\): window id # (0x[[:xdigit:]]+)$'
  local activation='(^|[[:space:],])_NET_ACTIVE_WINDOW([[:space:],]|$)'
  command -v xprop >/dev/null && command -v timeout >/dev/null || return 1
  [[ $pid =~ ^[1-9][0-9]*$ && $attempts =~ ^[1-9][0-9]*$ ]] || return 1
  for ((i=0; i<attempts; i++)); do
    kill -0 "$pid" 2>/dev/null || return 1
    root=$(LC_ALL=C timeout 1s xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null) || root=''
    if [[ $root =~ $pattern ]]; then
      id=${BASH_REMATCH[1]}
      if [[ ! $id =~ ^0x0+$ ]]; then
        check=$(LC_ALL=C timeout 1s xprop -id "$id" _NET_SUPPORTING_WM_CHECK 2>/dev/null) || check=''
        supported=$(LC_ALL=C timeout 1s xprop -root _NET_SUPPORTED 2>/dev/null) || supported=''
        owner=$(LC_ALL=C timeout 1s xprop -root _OPENBOX_PID 2>/dev/null) || owner=''
        if [[ $check == "$root" && $supported == '_NET_SUPPORTED(ATOM) = '* && $supported =~ $activation && $owner == "_OPENBOX_PID(CARDINAL) = $pid" ]] && kill -0 "$pid" 2>/dev/null; then
          [[ $previous == "$id" ]] && return 0
          previous=$id
          sleep .25
          continue
        fi
      fi
    fi
    previous=''
    sleep .25
  done
  return 1
}
