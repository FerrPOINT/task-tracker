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

package botmarble

import (
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/marble"
	"github.com/FerrPOINT/task-tracker/pkg/user"
)

// botColors is a cool-toned palette distinct from the marble default so bot avatars are visually recognizable as bots at a glance.
var botColors = []string{
	"#3B82F6",
	"#06B6D4",
	"#8B5CF6",
	"#14B8A6",
	"#6366F1",
}

// Provider renders marble-style avatars using the bot-specific palette.
type Provider struct{}

func (p *Provider) GetAvatar(u *user.User, size int64) ([]byte, string, error) {
	return marble.GenerateSVG(u, size, botColors)
}

func (p *Provider) AsDataURI(u *user.User, size int64) (string, error) {
	return marble.GenerateDataURI(u, size, botColors)
}

func (p *Provider) FlushCache(_ *user.User) error { return nil }
