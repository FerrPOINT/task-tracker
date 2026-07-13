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

package models

import (
	_ "github.com/FerrPOINT/task-tracker/pkg/config" // To trigger its init() which initializes the config
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/mail"
	"github.com/FerrPOINT/task-tracker/pkg/notifications"
)

// SetupTests takes care of seting up the db, fixtures etc.
// This is an extra function to be able to call the fixtures setup from the web tests.
func SetupTests() {
	var err error
	x, err = db.CreateTestEngine()
	if err != nil {
		log.Fatal(err)
	}

	tables := []interface{}{}
	tables = append(tables, GetTables()...)
	tables = append(tables, notifications.GetTables()...)

	err = x.Sync2(tables...)
	if err != nil {
		log.Fatal(err)
	}

	err = db.CreateParadeDBIndexes()
	if err != nil {
		log.Fatal(err)
	}

	err = db.InitTestFixtures(
		"files",
		"label_tasks",
		"labels",
		"link_shares",
		"projects",
		"task_assignees",
		"task_attachments",
		"task_comments",
		"task_relations",
		"task_reminders",
		"tasks",
		"time_entries",
		"team_projects",
		"team_members",
		"teams",
		"users",
		"user_tokens",
		"users_projects",
		"buckets",
		"saved_filters",
		"subscriptions",
		"favorites",
		"api_tokens",
		"reactions",
		"project_views",
		"task_positions",
		"task_buckets",
		"sessions",
		"webhooks",
		"totp",
		"oauth_codes",
		"notifications",
	)
	if err != nil {
		log.Fatal(err)
	}

	// Start the pseudo mail queue
	mail.StartMailDaemon()
}
