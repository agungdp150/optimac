package opti

import (
	"context"
	"encoding/json"
	"os/exec"
	"strings"
	"time"
)

// UpdateItem is a single package with an available newer version.
type UpdateItem struct {
	Name    string `json:"name"`
	Current string `json:"current"`
	Latest  string `json:"latest"`
	Kind    string `json:"kind"`
}

// UpdatesReport summarizes outdated software OptiMac can detect.
type UpdatesReport struct {
	BrewAvailable bool         `json:"brew_available"`
	Formulae      []UpdateItem `json:"formulae"`
	Casks         []UpdateItem `json:"casks"`
	Notes         []string     `json:"notes"`
}

// Total returns the number of outdated packages found.
func (r UpdatesReport) Total() int {
	return len(r.Formulae) + len(r.Casks)
}

// CheckUpdates reports outdated Homebrew formulae and casks. It reads the local
// Homebrew state (`brew outdated`) and never triggers a network `brew update`.
func CheckUpdates() (UpdatesReport, error) {
	report := UpdatesReport{}
	brew, err := exec.LookPath("brew")
	if err != nil {
		report.Notes = append(report.Notes, "Homebrew not found in PATH; skipping formula and cask checks.")
		return report, nil
	}
	report.BrewAvailable = true

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	out, err := exec.CommandContext(ctx, brew, "outdated", "--greedy", "--json=v2").Output()
	if err != nil {
		report.Notes = append(report.Notes, "Could not read `brew outdated`: "+strings.TrimSpace(err.Error()))
		return report, nil
	}

	var payload struct {
		Formulae []struct {
			Name              string   `json:"name"`
			InstalledVersions []string `json:"installed_versions"`
			CurrentVersion    string   `json:"current_version"`
		} `json:"formulae"`
		Casks []struct {
			Name              string   `json:"name"`
			InstalledVersions []string `json:"installed_versions"`
			CurrentVersion    string   `json:"current_version"`
		} `json:"casks"`
	}
	if err := json.Unmarshal(out, &payload); err != nil {
		report.Notes = append(report.Notes, "Could not parse Homebrew output: "+err.Error())
		return report, nil
	}

	for _, f := range payload.Formulae {
		report.Formulae = append(report.Formulae, UpdateItem{
			Name:    f.Name,
			Current: strings.Join(f.InstalledVersions, ", "),
			Latest:  f.CurrentVersion,
			Kind:    "formula",
		})
	}
	for _, c := range payload.Casks {
		report.Casks = append(report.Casks, UpdateItem{
			Name:    c.Name,
			Current: strings.Join(c.InstalledVersions, ", "),
			Latest:  c.CurrentVersion,
			Kind:    "cask",
		})
	}
	if report.Total() == 0 {
		report.Notes = append(report.Notes, "Everything Homebrew manages is up to date.")
	} else {
		report.Notes = append(report.Notes, "Run `brew upgrade` to apply these updates.")
	}
	return report, nil
}
