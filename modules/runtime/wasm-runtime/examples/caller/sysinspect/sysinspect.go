// Package log provides a way to run system inspection commands
// on the host system from within a WebAssembly module.
package api

import (
	"encoding/json"
	"fmt"
	"unsafe"
)

//go:wasmimport api exec
func __execJSON(reqPtr, reqLen, outPtr, outCap uint32) int32

//go:wasmimport api log
func __hostLog(level int32, msgPtr, msgLen uint32)

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

	n := __execJSON(
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

const (
	Debug = 0
	Info  = 1
	Warn  = 2
	Error = 3
)

// Log sends a formatted log line to the host runtime.
func Log(level int32, format string, args ...any) {
	msg := fmt.Sprintf(format, args...)
	if msg == "" {
		return
	}
	b := []byte(msg)
	__hostLog(
		level,
		uint32(uintptr(unsafe.Pointer(&b[0]))),
		uint32(len(b)),
	)
}
