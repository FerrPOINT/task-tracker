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
	"github.com/FerrPOINT/task-tracker/pkg/files"
	"github.com/FerrPOINT/task-tracker/pkg/initialize"
	"github.com/FerrPOINT/task-tracker/pkg/log"

	"github.com/spf13/cobra"
)

func init() {
	repairFileMimeTypesCmd.Flags().Bool("dry-run", false, "Preview repairs without making changes")
	repairCmd.AddCommand(repairFileMimeTypesCmd)
}

var repairFileMimeTypesCmd = &cobra.Command{
	Use:   "file-mime-types",
	Short: "Detect and set MIME types for all files that have none",
	Long: `Scans all files in the database that have no MIME type set,
detects the type from the stored file content, and updates the database.

This is useful after upgrading from a version that did not store MIME types
on file creation. Only files with an empty or NULL mime column are affected.

Use --dry-run to preview what would be fixed without making changes.`,
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

		result, err := files.RepairFileMimeTypes(s, dryRun)
		if err != nil {
			log.Errorf("Failed to repair file MIME types: %s", err)
			return
		}

		if !dryRun {
			if err := s.Commit(); err != nil {
				log.Errorf("Failed to commit changes: %s", err)
				return
			}
		}

		log.Infof("Repair complete:")
		log.Infof("  Files scanned: %d", result.Total)
		log.Infof("  Files updated: %d", result.Updated)

		if len(result.Errors) > 0 {
			log.Errorf("Errors encountered (%d):", len(result.Errors))
			for _, e := range result.Errors {
				log.Errorf("  - %s", e)
			}
		}

		if result.Total == 0 {
			log.Infof("No files with missing MIME types found - all files are healthy!")
		}
	},
}
