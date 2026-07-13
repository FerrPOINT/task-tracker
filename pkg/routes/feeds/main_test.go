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

package feeds

import (
	"os"
	"testing"

	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/events"
	"github.com/FerrPOINT/task-tracker/pkg/files"
	"github.com/FerrPOINT/task-tracker/pkg/i18n"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/models"
	"github.com/FerrPOINT/task-tracker/pkg/user"
)

func TestMain(m *testing.M) {
	log.InitLogger()
	config.InitDefaultConfig()
	i18n.Init()
	files.InitTests()
	user.InitTests()
	models.SetupTests()
	events.Fake()
	os.Exit(m.Run())
}
