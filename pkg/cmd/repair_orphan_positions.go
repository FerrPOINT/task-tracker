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
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/initialize"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/models"

	"github.com/spf13/cobra"
)

func init() {
	repairOrphanPositionsCmd.Flags().Bool("dry-run", false, "Preview repairs without making changes")
	repairCmd.AddCommand(repairOrphanPositionsCmd)
}

var repairOrphanPositionsCmd = &cobra.Command{
	Use:   "orphan-positions",
	Short: "Remove orphaned task position records for deleted tasks or views",
	Long: `Removes all task position records that reference tasks or project views
which no longer exist in the database.

This can happen when tasks or views are deleted but their position records
are not fully cleaned up.

Use --dry-run to preview what would be deleted without making changes.`,
	PreRun: func(_ *cobra.Command, _ []string) {
		initialize.FullInitWithoutAsync()
	},
	Run: func(cmd *cobra.Command, _ []string) {
		dryRun, _ := cmd.Flags().GetBool("dry-run")

		s := db.NewSession()
		defer s.Close()

		if dryRun {
			log.Infof("Running in dry-run mode - no changes will be made")
		}

		count, err := models.DeleteOrphanedTaskPositions(s, dryRun)
		if err != nil {
			log.Errorf("Could not delete orphaned task positions: %s", err)
			return
		}

		if !dryRun {
			if err := s.Commit(); err != nil {
				log.Errorf("Could not commit orphaned task position deletion: %s", err)
				return
			}
		}

		if count == 0 {
			log.Infof("No orphaned task positions found.")
			return
		}

		if dryRun {
			log.Infof("Would delete %d orphaned task positions.", count)
		} else {
			log.Infof("Successfully deleted %d orphaned task positions.", count)
		}
	},
}
