// Task Tracker is a self-hosted task and kanban board application.
// Copyright 2026-present Task Tracker and contributors. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

package cmd

import (
	"github.com/FerrPOINT/task-tracker/pkg/initialize"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/modules/dump"
	"github.com/spf13/cobra"
)

var preserveConfig bool

func init() {
	rootCmd.AddCommand(restoreCmd)
	restoreCmd.Flags().BoolVar(&preserveConfig, "preserve-config", false, "Preserve existing configuration instead of restoring from dump")
}

var restoreCmd = &cobra.Command{
	Use:   "restore [filename]",
	Short: "Restores all vikunja data from a vikunja dump.",
	Args:  cobra.ExactArgs(1),
	PreRun: func(_ *cobra.Command, _ []string) {
		initialize.FullInitWithoutAsync()
	},
	Run: func(_ *cobra.Command, args []string) {
		if err := dump.Restore(args[0], !preserveConfig); err != nil {
			log.Critical(err.Error())
		}
	},
}
