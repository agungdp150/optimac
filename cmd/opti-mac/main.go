package main

import (
	"fmt"
	"os"

	"github.com/luceid/opti-mac/internal/cli"
)

var version = "0.1.0"

func main() {
	if err := cli.Run(os.Args[1:], version); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}
