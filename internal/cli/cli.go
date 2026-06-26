package cli

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"time"

	"github.com/agungdp150/optimac/internal/opti"
	"github.com/agungdp150/optimac/internal/tui"
	tea "github.com/charmbracelet/bubbletea"
)

func Run(args []string, version string) error {
	return run(args, version, os.Stdout)
}

func run(args []string, version string, out io.Writer) error {
	if len(args) == 0 {
		return runMenu()
	}

	switch args[0] {
	case "help", "-h", "--help":
		printHelp(out)
		return nil
	case "menu", "ui":
		return runMenu()
	case "version", "--version":
		fmt.Fprintln(out, version)
		return nil
	case "scan", "smart-scan":
		return runScan(args[1:], out)
	case "clean":
		return runClean(args[1:], out)
	case "analyze":
		return runAnalyze(args[1:], out)
	case "duplicates":
		return runDuplicates(args[1:], out)
	case "optimize":
		return runOptimize(args[1:], out)
	case "status":
		return runStatus(args[1:], out)
	case "artifacts":
		return runArtifacts(args[1:], out)
	case "updates":
		return runUpdates(args[1:], out)
	case "apps":
		return runApps(args[1:], out)
	case "uninstall":
		return runUninstall(args[1:], out)
	case "browser":
		return runBrowser(args[1:], out)
	case "login", "startup":
		return runLogin(args[1:], out)
	case "threats", "scan-threats":
		return runThreats(args[1:], out)
	case "space", "hidden":
		return runSpace(args[1:], out)
	case "orphans":
		return runOrphans(args[1:], out)
	case "doctor", "health":
		return runDoctor(args[1:], out)
	case "restore":
		return runRestore(args[1:], out)
	case "log", "history":
		return runLog(args[1:], out)
	case "trash":
		return runTrash(args[1:], out)
	case "config":
		return runConfig(args[1:], out)
	default:
		return fmt.Errorf("unknown command %q", args[0])
	}
}

func printHelp(out io.Writer) {
	fmt.Fprintln(out, `OptiMac - safe macOS cleanup and maintenance CLI

Usage:
  opti-mac                      Open the interactive terminal menu
  opti-mac menu                 Open the interactive terminal menu
  opti-mac scan                 Run a safe summary scan
  opti-mac clean [--execute]    Deep clean caches, logs, temp, and dev files
  opti-mac analyze [path]       Show largest files in a path
  opti-mac duplicates [path]    Find duplicate files by SHA-256, any size
  opti-mac artifacts [path]     Find regenerable build/dependency dirs
  opti-mac apps                 List installed applications by size
  opti-mac uninstall <app>      Remove an app and its leftover files
  opti-mac browser [name]       Clear browser caches, history, and cookies
  opti-mac updates              Show outdated Homebrew formulae and casks
  opti-mac login [list]         List or toggle login items and launch agents
  opti-mac threats              Scan for known adware/PUP signatures
  opti-mac space                Find hidden space (snapshots, backups, caches)
  opti-mac orphans              Find support data left by uninstalled apps
  opti-mac doctor               Read-only system security/capacity check
  opti-mac optimize --ram       Purge inactive memory
  opti-mac status               Show system status
  opti-mac log                  Show recorded cleanup operations
  opti-mac restore <id>         Restore files from a cleanup operation
  opti-mac trash empty          Permanently delete trashed files
  opti-mac config [init|path]   Show or create the config file
  opti-mac version              Print version

Safety:
  Destructive commands preview by default. Add --execute only after reviewing the output.
  clean deletes regenerable caches permanently so space is freed immediately; pass
  clean --trash to keep a restore point instead. uninstall/browser move to the
  trash by default and can be undone with 'restore'.
  Use clean --execute --sudo only when normal cleanup reports permission errors.`)
}

func runMenu() error {
	program := tea.NewProgram(tui.New(), tea.WithAltScreen())
	_, err := program.Run()
	return err
}

func runScan(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("scan", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	clean, err := opti.ScanCleanable()
	if err != nil {
		return err
	}
	status, err := opti.SystemStatus()
	if err != nil {
		return err
	}
	payload := map[string]any{
		"cleanable_items": len(clean.Items),
		"cleanable_bytes": clean.TotalBytes,
		"status":          status,
	}
	if *jsonOut {
		return writeJSON(out, payload)
	}
	fmt.Fprintf(out, "Cleanable: %s across %d items\n", opti.FormatBytes(clean.TotalBytes), len(clean.Items))
	fmt.Fprintf(out, "Disk: %s free of %s\n", opti.FormatBytesU(status.HomeDiskFree), opti.FormatBytesU(status.HomeDiskTotal))
	fmt.Fprintf(out, "Memory: %s used / %s total\n", opti.FormatBytesU(status.Memory.Used), opti.FormatBytesU(status.Memory.Total))
	return nil
}

func runClean(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("clean", flag.ContinueOnError)
	execute := fs.Bool("execute", false, "delete files")
	sudo := fs.Bool("sudo", false, "re-run cleanup through sudo")
	trash := fs.Bool("trash", false, "keep a restore point by moving files to the OptiMac trash instead of deleting (caches regenerate, so clean deletes by default)")
	olderThan := fs.String("older-than", "", "only clean items not modified within this window, e.g. 7d, 48h")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	minAge, err := parseAge(*olderThan)
	if err != nil {
		return err
	}
	if *sudo {
		if !*execute {
			return errors.New("--sudo is only supported with --execute")
		}
		if err := rerunWithSudo(args); err != nil {
			return err
		}
		if os.Geteuid() != 0 {
			return nil
		}
	}
	result, err := opti.Clean(opti.CleanOptions{Execute: *execute, NoTrash: !*trash, MinAge: minAge})
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, result)
	}
	if !*execute {
		fmt.Fprintln(out, "Dry run. Re-run with --execute to delete these files.")
	}
	printCleanTargets(out, result.Targets)
	for _, item := range result.Items {
		fmt.Fprintf(out, "%10s  %-12s  %s\n", opti.FormatBytes(item.Size), item.Kind, item.Path)
	}
	if *execute {
		printRemovalSummary(out, result)
		printCleanFailures(out, result.Failures)
		return nil
	}
	fmt.Fprintf(out, "Potential cleanup: %s across %d items\n", opti.FormatBytes(result.TotalBytes), len(result.Items))
	return nil
}

func printCleanTargets(out io.Writer, targets []opti.CleanTargetResult) {
	if len(targets) == 0 {
		return
	}
	fmt.Fprintln(out, "Checked targets:")
	emptyChecked := 0
	for _, target := range targets {
		if target.Status == "checked" && target.ItemCount == 0 {
			emptyChecked++
			continue
		}
		path := target.Path
		if path == "" {
			path = target.Pattern
		}
		detail := target.Status
		if target.Status == "checked" {
			detail = fmt.Sprintf("checked, %d items, %s", target.ItemCount, opti.FormatBytes(target.TotalBytes))
		}
		if target.Error != "" {
			detail += ": " + target.Error
		}
		fmt.Fprintf(out, "  %-18s %-18s %s\n", target.Kind, detail, path)
	}
	if emptyChecked > 0 {
		fmt.Fprintf(out, "  %-18s %d checked targets had no cleanable items\n", "empty", emptyChecked)
	}
	fmt.Fprintln(out)
}

func rerunWithSudo(args []string) error {
	if os.Geteuid() == 0 || os.Getenv("OPTI_MAC_ELEVATED") == "1" {
		return nil
	}
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	cmdArgs := append([]string{executable, "clean"}, args...)
	cmd := exec.Command("sudo", cmdArgs...)
	cmd.Env = append(os.Environ(), "OPTI_MAC_ELEVATED=1")
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func printCleanFailures(out io.Writer, failures []opti.CleanFailure) {
	if len(failures) == 0 {
		return
	}
	fmt.Fprintf(out, "Skipped %d items due to errors:\n", len(failures))
	for _, failure := range failures {
		fmt.Fprintf(out, "  %s: %s\n", failure.Path, failure.Error)
	}
}

func runAnalyze(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("analyze", flag.ContinueOnError)
	limit := fs.Int("limit", 20, "maximum items to print")
	min := fs.String("min-size", "10MB", "minimum file size")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	path := "."
	if fs.NArg() > 0 {
		path = fs.Arg(0)
	}
	minSize, err := parseSize(*min)
	if err != nil {
		return err
	}
	items, err := opti.AnalyzeLargest(path, *limit, minSize)
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, items)
	}
	for _, item := range items {
		fmt.Fprintf(out, "%10s  %s\n", opti.FormatBytes(item.Size), item.Path)
	}
	return nil
}

func runDuplicates(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("duplicates", flag.ContinueOnError)
	min := fs.String("min-size", "1B", "minimum file size")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	path := "."
	if fs.NArg() > 0 {
		path = fs.Arg(0)
	}
	minSize, err := parseSize(*min)
	if err != nil {
		return err
	}
	groups, err := opti.FindDuplicates(path, minSize)
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, groups)
	}
	for _, group := range groups {
		if group.Match == "name" {
			fmt.Fprintf(out, "%s similar name group in %d files\n", opti.FormatBytes(group.Size), len(group.Paths))
		} else {
			fmt.Fprintf(out, "%s exact duplicate in %d files\n", opti.FormatBytes(group.Size), len(group.Paths))
		}
		for _, path := range group.Paths {
			fmt.Fprintf(out, "  %s\n", path)
		}
	}
	return nil
}

func runOptimize(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("optimize", flag.ContinueOnError)
	ram := fs.Bool("ram", true, "purge inactive memory")
	sudo := fs.Bool("sudo", false, "run memory purge through sudo")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if !*ram {
		return errors.New("only --ram optimization is currently supported")
	}
	result, err := opti.OptimizeMemory(*sudo)
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, result)
	}
	fmt.Fprintf(out, "Memory before: %s used / %s free\n", opti.FormatBytesU(result.Before.Used), opti.FormatBytesU(result.Before.Free))
	fmt.Fprintf(out, "Memory after:  %s used / %s free\n", opti.FormatBytesU(result.After.Used), opti.FormatBytesU(result.After.Free))
	if result.Output != "" {
		fmt.Fprintln(out, result.Output)
	}
	return nil
}

func runStatus(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	status, err := opti.SystemStatus()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, status)
	}
	fmt.Fprintln(out, opti.StatusDashboard(status))
	return nil
}

func printRemovalSummary(out io.Writer, result opti.CleanResult) {
	if result.Trashed {
		fmt.Fprintf(out, "Moved %s across %d items to the trash\n", opti.FormatBytes(result.RemovedBytes), result.RemovedCount)
		if result.OperationID != "" {
			fmt.Fprintf(out, "Not freed yet — restore with 'opti-mac restore %s' or reclaim with 'opti-mac trash empty'\n", result.OperationID)
		}
		return
	}
	fmt.Fprintf(out, "Freed %s across %d items\n", opti.FormatBytes(result.RemovedBytes), result.RemovedCount)
}

func runArtifacts(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("artifacts", flag.ContinueOnError)
	execute := fs.Bool("execute", false, "remove the artifact directories")
	noTrash := fs.Bool("no-trash", false, "delete permanently instead of moving to trash")
	min := fs.String("min-size", "1MB", "minimum directory size")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	path := "."
	if fs.NArg() > 0 {
		path = fs.Arg(0)
	}
	minSize, err := parseSize(*min)
	if err != nil {
		return err
	}
	dirs, err := opti.FindArtifacts(path, minSize)
	if err != nil {
		return err
	}
	result, err := opti.PurgeArtifacts(path, dirs, opti.CleanOptions{Execute: *execute, NoTrash: *noTrash})
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, result)
	}
	for _, dir := range dirs {
		fmt.Fprintf(out, "%10s  %-22s  %s\n", opti.FormatBytes(dir.Size), dir.Kind, dir.Path)
	}
	if *execute {
		printRemovalSummary(out, result)
		printCleanFailures(out, result.Failures)
		return nil
	}
	fmt.Fprintf(out, "\nFound %s across %d artifact directories. Re-run with --execute to remove.\n", opti.FormatBytes(result.TotalBytes), len(dirs))
	return nil
}

func runUpdates(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("updates", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	report, err := opti.CheckUpdates()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, report)
	}
	printUpdateItems(out, "Formulae", report.Formulae)
	printUpdateItems(out, "Casks", report.Casks)
	for _, note := range report.Notes {
		fmt.Fprintln(out, note)
	}
	return nil
}

func printUpdateItems(out io.Writer, title string, items []opti.UpdateItem) {
	if len(items) == 0 {
		return
	}
	fmt.Fprintf(out, "%s (%d):\n", title, len(items))
	for _, item := range items {
		fmt.Fprintf(out, "  %-28s %s -> %s\n", item.Name, item.Current, item.Latest)
	}
}

func runApps(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("apps", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	all := fs.Bool("all", false, "include protected system applications")
	if err := fs.Parse(args); err != nil {
		return err
	}
	apps, err := opti.ListInstalledApps()
	if err != nil {
		return err
	}
	if !*all {
		filtered := apps[:0]
		for _, app := range apps {
			if !app.System {
				filtered = append(filtered, app)
			}
		}
		apps = filtered
	}
	if *jsonOut {
		return writeJSON(out, apps)
	}
	for _, app := range apps {
		tag := ""
		if app.System {
			tag = "  [system]"
		}
		fmt.Fprintf(out, "%10s  %-32s %s%s\n", opti.FormatBytes(app.Size), app.Name, app.BundleID, tag)
	}
	fmt.Fprintf(out, "\n%d applications. Remove one with: opti-mac uninstall \"<name>\"\n", len(apps))
	return nil
}

func runUninstall(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("uninstall", flag.ContinueOnError)
	execute := fs.Bool("execute", false, "remove the app and leftovers")
	noTrash := fs.Bool("no-trash", false, "delete permanently instead of moving to trash")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if fs.NArg() == 0 {
		return errors.New("usage: opti-mac uninstall <app name or path>")
	}
	plan, err := opti.PlanUninstall(fs.Arg(0))
	if err != nil {
		return err
	}
	if *execute && opti.ProcessRunning(plan.AppName) {
		fmt.Fprintf(out, "Warning: %s appears to be running. Quit it before uninstalling.\n", plan.AppName)
	}
	result, err := opti.ExecuteUninstall(plan, opti.CleanOptions{Execute: *execute, NoTrash: *noTrash})
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, map[string]any{"plan": plan, "result": result})
	}
	fmt.Fprintf(out, "%s  (%s)\n", plan.AppName, plan.BundleID)
	fmt.Fprintf(out, "%10s  %-14s  %s\n", opti.FormatBytes(plan.AppSize), "application", plan.AppPath)
	for _, leftover := range plan.Leftovers {
		fmt.Fprintf(out, "%10s  %-14s  %s\n", opti.FormatBytes(leftover.Size), leftover.Kind, leftover.Path)
	}
	if *execute {
		printRemovalSummary(out, result)
		printCleanFailures(out, result.Failures)
		return nil
	}
	fmt.Fprintf(out, "\nWould remove %s across %d items. Re-run with --execute.\n", opti.FormatBytes(plan.TotalBytes), len(result.Items))
	return nil
}

func runBrowser(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("browser", flag.ContinueOnError)
	execute := fs.Bool("execute", false, "clear the browser data")
	noTrash := fs.Bool("no-trash", false, "delete permanently instead of moving to trash")
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	browser := ""
	if fs.NArg() > 0 {
		browser = fs.Arg(0)
	}
	if *execute && browser != "" && opti.ProcessRunning(browser) {
		fmt.Fprintf(out, "Warning: %s appears to be running. Quit it first so cookies/history clear cleanly.\n", browser)
	}
	result, err := opti.CleanBrowsers(browser, opti.CleanOptions{Execute: *execute, NoTrash: *noTrash})
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, result)
	}
	for _, item := range result.Items {
		fmt.Fprintf(out, "%10s  %-22s  %s\n", opti.FormatBytes(item.Size), item.Kind, item.Path)
	}
	if *execute {
		printRemovalSummary(out, result)
		printCleanFailures(out, result.Failures)
		return nil
	}
	fmt.Fprintf(out, "\nWould clear %s across %d items. Clearing history/cookies signs you out. Re-run with --execute.\n", opti.FormatBytes(result.TotalBytes), len(result.Items))
	return nil
}

func runLogin(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("login", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	action := "list"
	if fs.NArg() > 0 {
		action = fs.Arg(0)
	}
	switch action {
	case "list":
		items, err := opti.ListLaunchItems()
		if err != nil {
			return err
		}
		if *jsonOut {
			return writeJSON(out, items)
		}
		for _, item := range items {
			state := "enabled"
			if item.Disabled {
				state = "disabled"
			}
			fmt.Fprintf(out, "%-14s %-9s %-40s %s\n", item.Scope, state, item.Label, item.Program)
		}
		return nil
	case "disable", "enable":
		if fs.NArg() < 2 {
			return fmt.Errorf("usage: opti-mac login %s <label>", action)
		}
		item, err := opti.FindLaunchItem(fs.Arg(1))
		if err != nil {
			return err
		}
		if err := opti.SetLaunchItemEnabled(item, action == "enable"); err != nil {
			return err
		}
		fmt.Fprintf(out, "%sd %s\n", action, item.Label)
		return nil
	default:
		return fmt.Errorf("unknown login action %q (use list, enable, or disable)", action)
	}
}

func runSpace(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("space", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	report, err := opti.ScanHiddenSpace()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, report)
	}
	for _, item := range report.Items {
		flag := " "
		if item.Removable {
			flag = "✓"
		}
		fmt.Fprintf(out, "%s %10s  %-22s %s\n", flag, opti.FormatBytes(item.Size), item.Label, item.Detail)
		if item.Hint != "" {
			fmt.Fprintf(out, "             reclaim: %s\n", item.Hint)
		}
	}
	if report.Snapshot.Count > 0 {
		fmt.Fprintf(out, "\n%d APFS local snapshots present (purgeable, not counted above)\n", report.Snapshot.Count)
	}
	for _, note := range report.Notes {
		fmt.Fprintln(out, note)
	}
	return nil
}

func runOrphans(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("orphans", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	report, err := opti.ScanOrphans()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, report)
	}
	for _, item := range report.Items {
		fmt.Fprintf(out, "%10s  %-13s %s\n", opti.FormatBytes(item.Size), item.Kind, item.Identifier)
		fmt.Fprintf(out, "             %s\n", item.Path)
	}
	if len(report.Items) > 0 {
		fmt.Fprintf(out, "\nTotal: %s across %d orphaned items\n", opti.FormatBytes(report.TotalBytes), len(report.Items))
	}
	for _, note := range report.Notes {
		fmt.Fprintln(out, note)
	}
	return nil
}

func runDoctor(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("doctor", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	report, err := opti.SystemHealth()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, report)
	}
	for _, check := range report.Checks {
		mark := "•"
		switch check.Status {
		case "ok":
			mark = "✓"
		case "warn":
			mark = "!"
		}
		fmt.Fprintf(out, " %s  %-28s %s\n", mark, check.Name, check.Detail)
	}
	ok, warn, info := report.Counts()
	fmt.Fprintf(out, "\n%d ok · %d warnings · %d info\n", ok, warn, info)
	return nil
}

func runThreats(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("threats", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	report, err := opti.ScanThreats()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, report)
	}
	for _, match := range report.Matches {
		fmt.Fprintf(out, "[%s] %s  (%s)\n", match.Signature, match.Path, match.Detail)
	}
	for _, note := range report.Notes {
		fmt.Fprintln(out, note)
	}
	return nil
}

func runRestore(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("restore", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if fs.NArg() == 0 {
		return errors.New("usage: opti-mac restore <operation id>  (see opti-mac log)")
	}
	result, err := opti.RestoreOperation(fs.Arg(0))
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, result)
	}
	fmt.Fprintf(out, "Restored %d items (%s)\n", result.Restored, opti.FormatBytes(result.Bytes))
	for _, failure := range result.Failures {
		fmt.Fprintf(out, "  skipped %s: %s\n", failure.Path, failure.Error)
	}
	return nil
}

func runLog(args []string, out io.Writer) error {
	fs := flag.NewFlagSet("log", flag.ContinueOnError)
	jsonOut := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}
	ops, err := opti.ListOperations()
	if err != nil {
		return err
	}
	if *jsonOut {
		return writeJSON(out, ops)
	}
	if len(ops) == 0 {
		fmt.Fprintln(out, "No recorded operations.")
		return nil
	}
	for i := len(ops) - 1; i >= 0; i-- {
		op := ops[i]
		fmt.Fprintf(out, "%s  %-18s  %d items  %s\n", op.ID, op.Command, len(op.Items), opti.FormatBytes(op.TotalBytes()))
	}
	fmt.Fprintln(out, "\nRestore with: opti-mac restore <id>")
	return nil
}

func runTrash(args []string, out io.Writer) error {
	action := "status"
	if len(args) > 0 {
		action = args[0]
	}
	switch action {
	case "empty":
		freed, err := opti.EmptyTrash()
		if err != nil {
			return err
		}
		fmt.Fprintf(out, "Emptied trash, freed %s\n", opti.FormatBytes(freed))
		return nil
	case "status":
		ops, err := opti.ListOperations()
		if err != nil {
			return err
		}
		var total int64
		for _, op := range ops {
			total += op.TotalBytes()
		}
		fmt.Fprintf(out, "Trash holds %s across %d operations\n", opti.FormatBytes(total), len(ops))
		return nil
	default:
		return fmt.Errorf("unknown trash action %q (use status or empty)", action)
	}
}

func runConfig(args []string, out io.Writer) error {
	action := "show"
	if len(args) > 0 {
		action = args[0]
	}
	switch action {
	case "path":
		path, err := opti.ConfigPath()
		if err != nil {
			return err
		}
		fmt.Fprintln(out, path)
		return nil
	case "init":
		path, err := opti.SaveConfig(opti.DefaultConfig())
		if err != nil {
			return err
		}
		fmt.Fprintf(out, "Wrote default config to %s\n", path)
		return nil
	case "show":
		cfg, err := opti.LoadConfig()
		if err != nil {
			return err
		}
		return writeJSON(out, cfg)
	default:
		return fmt.Errorf("unknown config action %q (use show, path, or init)", action)
	}
}

func writeJSON(out io.Writer, value any) error {
	encoder := json.NewEncoder(out)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}

// parseAge parses a duration window like "7d", "48h", "30m". An empty string
// means no age filter.
func parseAge(value string) (time.Duration, error) {
	value = strings.TrimSpace(strings.ToLower(value))
	if value == "" {
		return 0, nil
	}
	if strings.HasSuffix(value, "d") {
		days, err := strconv.ParseFloat(strings.TrimSuffix(value, "d"), 64)
		if err != nil {
			return 0, err
		}
		return time.Duration(days * float64(24*time.Hour)), nil
	}
	return time.ParseDuration(value)
}

func parseSize(value string) (int64, error) {
	value = strings.TrimSpace(strings.ToUpper(value))
	multiplier := int64(1)
	units := []struct {
		suffix string
		factor int64
	}{
		{"TB", 1024 * 1024 * 1024 * 1024},
		{"GB", 1024 * 1024 * 1024},
		{"MB", 1024 * 1024},
		{"KB", 1024},
		{"B", 1},
	}
	for _, unit := range units {
		suffix := unit.suffix
		if strings.HasSuffix(value, suffix) {
			multiplier = unit.factor
			value = strings.TrimSpace(strings.TrimSuffix(value, suffix))
			break
		}
	}
	if value == "" {
		return 0, errors.New("missing size")
	}
	n, err := strconv.ParseFloat(value, 64)
	if err != nil {
		return 0, err
	}
	return int64(n * float64(multiplier)), nil
}
