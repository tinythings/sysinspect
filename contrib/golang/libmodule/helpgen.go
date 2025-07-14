// Package libmodule provides a library for generating help documentation.
package libmodule

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/fatih/color"
	"gopkg.in/yaml.v3"
)

type Option struct {
	Name        string `yaml:"name"`
	Description string `yaml:"description"`
}

type Argument struct {
	Name        string `yaml:"name"`
	Type        string `yaml:"type"`
	Required    bool   `yaml:"required"`
	Description string `yaml:"description"`
}

type Example struct {
	Description string `yaml:"description"`
	Code        string `yaml:"code"`
}

type Returns struct {
	Description string                 `yaml:":description"`
	Retcode     int                    `yaml:"retcode"`
	Message     string                 `yaml:"message"`
	Data        map[string]interface{} `yaml:"data"`
}

type ModuleDoc struct {
	Name        string     `yaml:"name"`
	Version     string     `yaml:"version"`
	Author      string     `yaml:"author"`
	Description string     `yaml:"description"`
	Options     []Option   `yaml:"options"`
	Arguments   []Argument `yaml:"arguments"`
	Examples    []Example  `yaml:"examples"`
	Returns     Returns    `yaml:"returns"`
}

func PrintModuleHelp(modDocYaml []byte) {
	var doc ModuleDoc

	if err := yaml.Unmarshal(modDocYaml, &doc); err != nil {
		fmt.Fprintln(os.Stderr, "Failed to load help:", err)
		os.Exit(2)
	}

	title := color.New(color.FgHiYellow)
	optarg := color.New(color.Bold, color.FgMagenta)
	oahelp := color.New(color.FgWhite)
	helpdescr := color.New(color.FgYellow)

	// Header: Name and version, Author, Description
	fmt.Printf("%s, %s (Author: %s)\n\n%s\n\n  %s\n\n",
		color.New(color.Bold, color.FgHiWhite).Sprintf("%s", doc.Name),
		color.New(color.Bold, color.FgHiGreen).Sprintf("%s", doc.Version),
		color.New(color.FgHiWhite).Sprintf("%s", doc.Author),
		title.Sprintf("%s", "Description:"),
		color.New(color.FgYellow).Sprintf("%s", doc.Description),
	)

	fmt.Printf(title.Sprintf("%s", "Options:\n\n"))
	for _, opt := range doc.Options {
		fmt.Printf("  %s\n    %s\n\n",
			optarg.Sprintf("%s", opt.Name),
			oahelp.Sprintf("%s", opt.Description),
		)
	}

	fmt.Printf(title.Sprintf("%s", "\nKeyword arguments:\n\n"))
	for _, arg := range doc.Arguments {
		fmt.Printf("  %s (type: %s, required: %s)\n    %s\n\n",
			optarg.Sprintf("%s", arg.Name),
			color.New(color.FgCyan).Sprintf("%s", arg.Type),
			color.New(color.FgHiRed).Sprintf("%v", arg.Required),
			oahelp.Sprintf("%s", arg.Description))
	}

	fmt.Println(title.Sprintf("\n%s", "Usage examples:\n"))
	for _, ex := range doc.Examples {
		oahelp.Printf("  %s:\n%s\n", helpdescr.Sprintf("%s", ex.Description), ex.Code)
	}

	fmt.Println(title.Sprintf("\n%s", "Returned data structure:\n"))
	dataJSON, err := json.MarshalIndent(doc.Returns.Data, "        ", "  ")
	if err != nil {
		fmt.Printf("  Failed to encode data as JSON: %v\n", err)
	} else {
		fmt.Printf("  %s\n{\n  \"retcode\": %s,\n  \"message\": %s,\n  \"data\": %s\n}\n",
			helpdescr.Sprintf("%s", doc.Returns.Description),
			fmt.Sprintf("%d", doc.Returns.Retcode),
			fmt.Sprintf("\"%s\"", doc.Returns.Message),
			string(dataJSON),
		)
	}
}
