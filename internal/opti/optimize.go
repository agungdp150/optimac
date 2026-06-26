package opti

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strings"
)

type OptimizeMemoryResult struct {
	Before MemoryStatus
	After  MemoryStatus
	Output string
}

func OptimizeMemory(useSudo bool) (OptimizeMemoryResult, error) {
	var result OptimizeMemoryResult
	if runtime.GOOS != "darwin" {
		return result, fmt.Errorf("memory optimization is only supported on macOS")
	}
	if _, err := exec.LookPath("purge"); err != nil {
		return result, fmt.Errorf("macOS purge command is not available")
	}

	before, err := SystemStatus()
	if err == nil {
		result.Before = before.Memory
	}

	_ = exec.Command("sync").Run()

	output, err := runPurge(useSudo)
	if err != nil {
		message := strings.TrimSpace(output)
		if message == "" {
			message = err.Error()
		}
		return result, fmt.Errorf("purge failed: %s", message)
	}

	after, err := SystemStatus()
	if err == nil {
		result.After = after.Memory
	}
	result.Output = strings.TrimSpace(output)
	return result, nil
}

func runPurge(useSudo bool) (string, error) {
	if !useSudo || os.Geteuid() == 0 {
		return runCommand("purge")
	}
	return runCommand("osascript", "-e", `do shell script "purge" with administrator privileges`)
}

func runCommand(name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &out
	cmd.Stdin = os.Stdin
	err := cmd.Run()
	return out.String(), err
}
