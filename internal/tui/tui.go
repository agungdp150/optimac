package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"

	"github.com/agungdp150/optimac/internal/opti"
)

type screen int

const (
	screenMenu screen = iota
	screenResult
	screenConfirmClean
	screenLiveStatus
	screenLargeFiles
	screenConfirmLargeDelete
	screenApps
	screenUninstallPreview
	screenDuplicates
	screenConfirmDuplicateDelete
)

type menuItem struct {
	icon        string
	title       string
	description string
	command     string
	action      action
}

type action int

const (
	actionScan action = iota
	actionCleanPreview
	actionCleanExecute
	actionAnalyzeDownloads
	actionDuplicatesDownloads
	actionArtifacts
	actionUpdates
	actionLoginItems
	actionThreats
	actionSpace
	actionOrphans
	actionDoctor
	actionApps
	actionOptimizeRAM
	actionStatus
	actionQuit
)

type model struct {
	cursor              int
	scroll              int
	frame               int
	width               int
	height              int
	screen              screen
	busy                bool
	title               string
	body                string
	systemLine          string
	liveBody            string
	liveUpdated         string
	livePaused          bool
	liveLoading         bool
	err                 error
	liveErr             error
	items               []menuItem
	diskLocations       []opti.DiskLocation
	diskCursor          int
	diskFree            uint64
	diskLoading         bool
	diskErr             error
	diskCached          bool
	largeItems          []opti.AnalyzeItem
	largeCursor         int
	largeScroll         int
	largeSelected       map[int]bool
	largeLocation       opti.DiskLocation
	largeExploring      bool
	largeLoading        bool
	largeScanned        bool
	largeSortMode       int
	largeStatus         string
	largeErr            error
	largeCache          map[string][]opti.AnalyzeItem
	duplicateGroups     []opti.DuplicateGroup
	duplicateRoot       string
	duplicateCursor     int
	duplicateScroll     int
	duplicateOpen       bool
	duplicateFileCursor int
	duplicateFileScroll int
	duplicateSelected   map[string]bool
	duplicatePermanent  bool
	duplicateSortMode   int
	duplicateLoading    bool
	duplicateErr        error
	duplicateStatus     string
	apps                []opti.InstalledApp
	appsCursor          int
	appsScroll          int
	appsLoading         bool
	appsErr             error
	appsStatus          string
	uninstallPlan       opti.UninstallPlan
	uninstallBusy       bool
}

type resultMsg struct {
	title string
	body  string
	err   error
}

type systemLineMsg string

type tickMsg time.Time

type liveStatusMsg struct {
	body    string
	updated string
	err     error
}

type liveStatusTickMsg time.Time

type largeFilesMsg struct {
	items []opti.AnalyzeItem
	err   error
}

type diskLocationsMsg struct {
	locations []opti.DiskLocation
	free      uint64
	err       error
}

type largeDeleteMsg struct {
	result opti.CleanResult
}

type duplicatesMsg struct {
	groups []opti.DuplicateGroup
	err    error
}

type duplicateDeleteMsg struct {
	result opti.CleanResult
}

type appsLoadedMsg struct {
	apps []opti.InstalledApp
	err  error
}

type uninstallPlanMsg struct {
	plan opti.UninstallPlan
	err  error
}

type uninstallResultMsg struct {
	app    string
	result opti.CleanResult
	err    error
}

func New() tea.Model {
	return model{
		screen: screenMenu,
		items: []menuItem{
			{icon: "◆", title: "Smart Scan", description: "Overview of cleanable space, disk, and memory.", command: "optimac scan", action: actionScan},
			{icon: "◈", title: "Deep Clean Preview", description: "Find user caches, logs, app states, dev caches, temp files.", command: "optimac clean", action: actionCleanPreview},
			{icon: "▲", title: "Run Deep Clean", description: "Delete user and sudo-only system cleanup targets after confirmation.", command: "optimac clean --execute --sudo", action: actionCleanExecute},
			{icon: "◇", title: "Large Files", description: "Choose a scope, then find, select, and delete large files.", command: "optimac analyze --limit 200 --min-size 50MB <scope>", action: actionAnalyzeDownloads},
			{icon: "◎", title: "Duplicates", description: "Find duplicate files in Downloads, including small files.", command: "optimac duplicates ~/Downloads", action: actionDuplicatesDownloads},
			{icon: "◢", title: "Project Artifacts", description: "Find regenerable build/dependency dirs in the current folder.", command: "optimac artifacts .", action: actionArtifacts},
			{icon: "◰", title: "Updates", description: "Show outdated Homebrew formulae and casks.", command: "optimac updates", action: actionUpdates},
			{icon: "◱", title: "Login Items", description: "List login items and launch agents that run automatically.", command: "optimac login list", action: actionLoginItems},
			{icon: "◭", title: "Threat Scan", description: "Scan for known adware and PUP signatures.", command: "optimac threats", action: actionThreats},
			{icon: "❖", title: "Hidden Space", description: "Find snapshots, backups, Docker, and app caches eating disk.", command: "optimac space", action: actionSpace},
			{icon: "⊘", title: "Orphans", description: "Find leftover data from apps that are no longer installed.", command: "optimac orphans", action: actionOrphans},
			{icon: "✚", title: "Doctor", description: "Read-only check of FileVault, firewall, disk, and memory.", command: "optimac doctor", action: actionDoctor},
			{icon: "⌫", title: "Uninstall Apps", description: "Browse installed apps by size and remove one with its leftovers.", command: "optimac apps", action: actionApps},
			{icon: "◌", title: "Optimize RAM", description: "Purge inactive memory. macOS may ask for an admin password.", command: "optimac optimize --ram --sudo", action: actionOptimizeRAM},
			{icon: "■", title: "System Status", description: "Show host, CPU, disk, and memory details.", command: "optimac status", action: actionStatus},
			{icon: "×", title: "Quit", description: "Close the OptiMac terminal UI.", command: "q", action: actionQuit},
		},
	}
}

func (m model) Init() tea.Cmd {
	return tea.Batch(loadSystemLine, spinnerTick())
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil
	case resultMsg:
		m.busy = false
		m.screen = screenResult
		m.title = msg.title
		m.body = msg.body
		m.err = msg.err
		m.scroll = 0
		return m, nil
	case systemLineMsg:
		m.systemLine = string(msg)
		return m, nil
	case tickMsg:
		m.frame++
		if m.busy || m.uninstallBusy || (m.screen == screenApps && m.appsLoading) || (m.screen == screenLiveStatus && m.liveLoading) || (m.screen == screenLargeFiles && (m.largeLoading || m.diskLoading)) || (m.screen == screenDuplicates && m.duplicateLoading) {
			return m, spinnerTick()
		}
		return m, nil
	case liveStatusMsg:
		m.liveUpdated = msg.updated
		if msg.err != nil {
			if m.liveBody == "" {
				m.liveErr = msg.err
			}
		} else {
			m.liveBody = msg.body
			m.liveErr = nil
		}
		m.liveLoading = false
		if m.screen == screenLiveStatus && !m.livePaused {
			return m, liveStatusTick()
		}
		return m, nil
	case liveStatusTickMsg:
		if m.screen == screenLiveStatus && !m.livePaused {
			return m, loadLiveStatus
		}
		return m, nil
	case diskLocationsMsg:
		m.diskLocations = msg.locations
		m.diskFree = msg.free
		m.diskErr = msg.err
		m.diskLoading = false
		m.diskCached = msg.err == nil
		m.largeExploring = false
		if msg.err != nil {
			m.largeStatus = "Analyze failed"
		} else {
			m.largeStatus = "Select a location to explore · quick estimates"
		}
		return m, nil
	case largeFilesMsg:
		m.largeItems = sortLargeItems(msg.items, m.largeSortMode)
		m.largeErr = msg.err
		m.largeLoading = false
		m.largeScanned = true
		m.largeCursor = 0
		m.largeScroll = 0
		m.largeSelected = make(map[int]bool)
		if msg.err != nil {
			m.largeStatus = "Scan failed"
		} else {
			if m.largeCache == nil {
				m.largeCache = make(map[string][]opti.AnalyzeItem)
			}
			m.largeCache[largeLocationCacheKey(m.largeLocation)] = append([]opti.AnalyzeItem(nil), m.largeItems...)
			m.largeStatus = "Found large files in " + m.largeLocation.Label
		}
		return m, nil
	case largeDeleteMsg:
		m.largeLoading = false
		m.applyLargeDeleteResult(msg.result)
		return m, nil
	case duplicatesMsg:
		m.duplicateLoading = false
		m.duplicateErr = msg.err
		m.duplicateGroups = sortDuplicateGroups(msg.groups, m.duplicateSortMode)
		m.duplicateCursor = 0
		m.duplicateScroll = 0
		m.duplicateOpen = false
		m.duplicateFileCursor = 0
		m.duplicateFileScroll = 0
		m.duplicateSelected = make(map[string]bool)
		if msg.err != nil {
			m.duplicateStatus = "Duplicate scan failed"
		} else {
			m.duplicateStatus = "Select a group to review files"
		}
		return m, nil
	case duplicateDeleteMsg:
		m.duplicateLoading = false
		m.applyDuplicateDeleteResult(msg.result)
		return m, nil
	case appsLoadedMsg:
		m.appsLoading = false
		m.apps = msg.apps
		m.appsErr = msg.err
		m.appsCursor = 0
		m.appsScroll = 0
		if msg.err != nil {
			m.appsStatus = "Failed to list applications"
		} else {
			m.appsStatus = "Select a removable app to uninstall · protected system apps are listed"
		}
		return m, nil
	case uninstallPlanMsg:
		m.appsLoading = false
		if msg.err != nil {
			m.appsErr = msg.err
			m.appsStatus = "Could not read app: " + msg.err.Error()
			m.screen = screenApps
			return m, nil
		}
		m.uninstallPlan = msg.plan
		m.screen = screenUninstallPreview
		return m, nil
	case uninstallResultMsg:
		m.uninstallBusy = false
		m.busy = false
		m.screen = screenResult
		m.title = "Uninstall " + msg.app
		m.scroll = 0
		if msg.err != nil {
			m.err = msg.err
			return m, nil
		}
		m.err = nil
		m.body = uninstallResultBody(msg.app, msg.result)
		return m, nil
	case tea.KeyMsg:
		key := msg.String()
		if key == "ctrl+c" {
			return m, tea.Quit
		}
		if m.busy {
			return m, nil
		}
		switch m.screen {
		case screenMenu:
			return m.updateMenu(key)
		case screenLiveStatus:
			return m.updateLiveStatus(key)
		case screenLargeFiles:
			return m.updateLargeFiles(key)
		case screenDuplicates:
			return m.updateDuplicates(key)
		case screenApps:
			return m.updateApps(key)
		case screenUninstallPreview:
			return m.updateUninstallPreview(key)
		case screenResult:
			switch key {
			case "up", "k":
				if m.scroll > 0 {
					m.scroll--
				}
				return m, nil
			case "down", "j":
				m.scroll++
				return m, nil
			case "pgup":
				m.scroll -= resultPageSize(m.height)
				if m.scroll < 0 {
					m.scroll = 0
				}
				return m, nil
			case "pgdown":
				m.scroll += resultPageSize(m.height)
				return m, nil
			case "home":
				m.scroll = 0
				return m, nil
			case "end":
				m.scroll = 1 << 30
				return m, nil
			case "esc", "b", "backspace", "enter":
				m.screen = screenMenu
				m.err = nil
				m.scroll = 0
				return m, nil
			case "q":
				return m, tea.Quit
			}
		case screenConfirmClean:
			switch key {
			case "y", "Y":
				m.busy = true
				m.screen = screenResult
				m.title = "Deep Clean"
				m.body = busyMessage(actionCleanExecute)
				return m, tea.Batch(runAction(actionCleanExecute), spinnerTick())
			case "n", "N", "esc", "backspace":
				m.screen = screenMenu
				return m, nil
			case "q":
				return m, tea.Quit
			}
		case screenConfirmLargeDelete:
			switch key {
			case "y", "Y":
				selected := m.selectedLargeFiles()
				m.screen = screenLargeFiles
				m.largeLoading = true
				m.largeStatus = "Moving selected files to OptiMac trash..."
				return m, tea.Batch(deleteLargeFiles(m.largeLocation, selected), spinnerTick())
			case "n", "N", "esc", "backspace":
				m.screen = screenLargeFiles
				return m, nil
			case "q":
				return m, tea.Quit
			}
		case screenConfirmDuplicateDelete:
			switch key {
			case "y", "Y":
				selected := m.selectedDuplicatePaths()
				m.screen = screenDuplicates
				m.duplicateLoading = true
				if m.duplicatePermanent {
					m.duplicateStatus = "Permanently deleting selected duplicates..."
				} else {
					m.duplicateStatus = "Moving selected duplicates to OptiMac trash..."
				}
				return m, tea.Batch(deleteDuplicates(m.duplicateRoot, selected, m.duplicatePermanent), spinnerTick())
			case "n", "N", "esc", "backspace":
				m.screen = screenDuplicates
				return m, nil
			case "q":
				return m, tea.Quit
			}
		}
	}
	return m, nil
}

func (m model) updateMenu(key string) (tea.Model, tea.Cmd) {
	switch key {
	case "up", "k":
		if m.cursor > 0 {
			m.cursor--
		}
	case "down", "j":
		if m.cursor < len(m.items)-1 {
			m.cursor++
		}
	case "home":
		m.cursor = 0
	case "end":
		m.cursor = len(m.items) - 1
	case "q":
		return m, tea.Quit
	case "enter":
		item := m.items[m.cursor]
		if item.action == actionQuit {
			return m, tea.Quit
		}
		if item.action == actionCleanExecute {
			m.screen = screenConfirmClean
			return m, nil
		}
		if item.action == actionStatus {
			m.screen = screenLiveStatus
			m.scroll = 0
			m.liveBody = "Collecting system, power, network, and process details..."
			m.liveUpdated = ""
			m.liveErr = nil
			m.livePaused = false
			m.liveLoading = true
			return m, tea.Batch(loadLiveStatus, spinnerTick())
		}
		if item.action == actionAnalyzeDownloads {
			m.screen = screenLargeFiles
			return m.openLargeFiles()
		}
		if item.action == actionDuplicatesDownloads {
			return m.openDuplicates()
		}
		if item.action == actionApps {
			return m.openApps()
		}
		m.busy = true
		m.screen = screenResult
		m.title = item.title
		m.body = busyMessage(item.action)
		return m, tea.Batch(runAction(item.action), spinnerTick())
	}
	return m, nil
}

func (m model) updateLiveStatus(key string) (tea.Model, tea.Cmd) {
	switch key {
	case "up", "k":
		if m.scroll > 0 {
			m.scroll--
		}
	case "down", "j":
		m.scroll++
	case "pgup":
		m.scroll -= liveStatusPageSize(m.height)
		if m.scroll < 0 {
			m.scroll = 0
		}
	case "pgdown":
		m.scroll += liveStatusPageSize(m.height)
	case "home":
		m.scroll = 0
	case "end":
		m.scroll = 1 << 30
	case " ", "space":
		m.livePaused = !m.livePaused
		if !m.livePaused {
			m.liveLoading = m.liveBody == ""
			if m.liveLoading {
				return m, tea.Batch(loadLiveStatus, spinnerTick())
			}
			return m, loadLiveStatus
		}
	case "r":
		m.liveLoading = m.liveBody == ""
		if m.liveLoading {
			return m, tea.Batch(loadLiveStatus, spinnerTick())
		}
		return m, loadLiveStatus
	case "esc", "b", "backspace", "enter":
		m.screen = screenMenu
		m.scroll = 0
		m.liveLoading = false
		return m, nil
	case "q":
		return m, tea.Quit
	}
	return m, nil
}
