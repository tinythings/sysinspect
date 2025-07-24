#!/usr/bin/jq -f

def oneof_null:
  if type == "object" then
    with_entries(
      if .key == "oneOf" and (.value | type == "array") then
        .value |= map(select(. != {"type": "null"})) | .value |= map(oneof_null)
      else
        .value |= oneof_null
      end
    )
  elif type == "array" then
    map(oneof_null)
  else
    .
  end;


def itemsfalse:
  if type == "object" then
    with_entries(
      select(.key != "items" or .value != false)
      | .value |= itemsfalse
    )
  elif type == "array" then
    map(itemsfalse)
  else
    .
  end;

itemsfalse | oneof_null
