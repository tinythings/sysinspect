package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"

	api "stuffspawner/sysinspect"
)

type Header struct {
	Opts []string               `json:"opts"`
	Args map[string]interface{} `json:"args"`
}

func readHeader() (Header, error) {
	in := bufio.NewScanner(os.Stdin)
	var hdr Header
	if !in.Scan() {
		return hdr, fmt.Errorf("missing header JSON")
	}
	if err := json.Unmarshal(in.Bytes(), &hdr); err != nil {
		return hdr, err
	}
	return hdr, nil
}

func doc() map[string]any {
	return map[string]any{
		"name": "caller", "version": "0.1.0", "author": "Bo Maryniuk",
		"description": "Executes `uname -a` via host syscall and returns stdout.",
		"arguments":   []any{}, "options": []any{},
		"examples": []any{
			map[string]any{"description": "Run uname", "code": `{ "args": {} }`},
			map[string]any{"description": "Show docs", "code": `{ "args": {}, "opts": ["man"] }`},
		},
		"returns": map[string]any{
			"description": "Returns stdout of uname -a",
			"sample":      map[string]any{"output": "Linux host ..."},
		},
	}
}

func main() {
	hdr, err := readHeader()
	if err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		os.Exit(1)
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetEscapeHTML(false)

	for _, o := range hdr.Opts {
		if o == "man" {
			_ = enc.Encode(doc())
			return
		}
	}

	out, err := api.Command("/usr/bin/uname", "-a").Output()
	api.Log(api.Info, "Called: \"uname -a\", output length: %d", len(out))
	if err != nil {
		_ = enc.Encode(map[string]any{"error": err.Error()})
		return
	}

	// Output structure
	_ = enc.Encode(map[string]any{"output": out})

	api.Log(api.Info, "Finished successfully")
}
