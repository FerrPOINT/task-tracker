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
	"time"

	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	_ "github.com/go-sql-driver/mysql" // Because.
	_ "github.com/lib/pq"              // Because.
	"xorm.io/xorm"

	_ "github.com/mattn/go-sqlite3" // Because.
)

var (
	x *xorm.Engine

	testCreatedTime time.Time
	testUpdatedTime time.Time
)

func init() {
	db.RegisterTables(GetTables())
}

// GetTables returns all structs which are also a table.
func GetTables() []interface{} {
	return []interface{}{
		&Project{},
		&Task{},
		&Team{},
		&TeamMember{},
		&TeamProject{},
		&ProjectUser{},
		&TaskAssginee{},
		&Label{},
		&LabelTask{},
		&TaskReminder{},
		&LinkSharing{},
		&TaskRelation{},
		&TaskAttachment{},
		&TaskComment{},
		&Bucket{},
		&UnsplashPhoto{},
		&SavedFilter{},
		&Subscription{},
		&Favorite{},
		&APIToken{},
		&Webhook{},
		&Reaction{},
		&ProjectView{},
		&TaskPosition{},
		&TaskBucket{},
		&TaskUnreadStatus{},
		&Session{},
		&OAuthCode{},
		&TimeEntry{},
	}
}

// SetEngine sets the xorm.Engine
func SetEngine() (err error) {
	x, err = db.CreateDBEngine()
	if err != nil {
		log.Criticalf("Could not connect to db: %v", err.Error())
		return
	}

	return nil
}

func getLimitFromPageIndex(page int, perPage int) (limit, start int) {

	// Get everything when page index is -1 or 0 (= not set)
	if page < 1 {
		return 0, 0
	}

	limit = config.ServiceMaxItemsPerPage.GetInt()
	if perPage > 0 {
		limit = perPage
	}

	start = limit * (page - 1)
	return
}

// GetTotalCount returns the total amount of something
func GetTotalCount(counting interface{}) (count int64, err error) {
	return x.Count(counting)
}
