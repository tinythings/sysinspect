// Package sysinspect provides a way to run system inspection commands
// on the host system from within a WebAssembly module.
package sysinspect

import (
	"encoding/json"
	"fmt"
	"unsafe"
)

//go:wasmimport api exec
func execJSON(reqPtr, reqLen, outPtr, outCap uint32) int32

type Cmd struct {
	Argv []string
	Cwd  string
}

func Command(name string, args ...string) *Cmd {
	argv := make([]string, 0, 1+len(args))
	argv = append(argv, name)
	argv = append(argv, args...)
	return &Cmd{Argv: argv}
}

func (c *Cmd) SetDir(dir string) *Cmd { c.Cwd = dir; return c }

// Output returns stdout; stderr is returned as error if exit_code != 0
func (c *Cmd) Output() (string, error) {
	req := map[string]any{"argv": c.Argv}
	if c.Cwd != "" {
		req["cwd"] = c.Cwd
	}

	reqb, err := json.Marshal(req)
	if err != nil {
		return "", err
	}

	out := make([]byte, 256*1024) // a buffer for host response

	n := execJSON(
		uint32(uintptr(unsafe.Pointer(&reqb[0]))),
		uint32(len(reqb)),
		uint32(uintptr(unsafe.Pointer(&out[0]))),
		uint32(len(out)),
	)
	if n < 0 {
		return "", fmt.Errorf("host exec failed (%d)", n)
	}

	var resp struct {
		ExitCode int    `json:"exit_code"`
		Stdout   string `json:"stdout"`
		Stderr   string `json:"stderr"`
	}
	if err := json.Unmarshal(out[:n], &resp); err != nil {
		return "", fmt.Errorf("bad host response json: %w", err)
	}

	if resp.ExitCode != 0 {
		return resp.Stdout, fmt.Errorf("exit %d: %s", resp.ExitCode, resp.Stderr)
	}
	return resp.Stdout, nil
}
