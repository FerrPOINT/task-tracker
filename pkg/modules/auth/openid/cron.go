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

package openid

import (
	"github.com/FerrPOINT/task-tracker/pkg/cron"
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/models"

	"xorm.io/builder"
	"xorm.io/xorm"
)

func RemoveEmptySSOTeams(s *xorm.Session) (err error) {
	teams := []*models.Team{}
	err = s.
		Where(
			builder.NotIn("id", builder.Expr("select team_members.team_id from team_members")),
			builder.Or(builder.Neq{"external_id": ""}, builder.NotNull{"external_id"}),
		).
		Find(&teams)
	if err != nil {
		return err
	}

	if len(teams) == 0 {
		return nil
	}

	teamIDs := make([]int64, 0, len(teams))
	for _, team := range teams {
		teamIDs = append(teamIDs, team.ID)
	}

	log.Debugf("Deleting empty teams: %v", teamIDs)

	_, err = s.In("id", teamIDs).Delete(&models.Team{})
	return err
}

func RegisterEmptyOpenIDTeamCleanupCron() {
	const logPrefix = "[Empty openid Team Cleanup Cron] "

	err := cron.Schedule("* * * * *", func() {
		s := db.NewSession()
		defer s.Close()

		err := RemoveEmptySSOTeams(s)
		if err != nil {
			log.Errorf(logPrefix+"Error removing empty openid team: %s", err)
			return
		}

		if err := s.Commit(); err != nil {
			log.Errorf(logPrefix+"Error committing: %s", err)
		}
	})
	if err != nil {
		log.Fatalf("Could not register empty openid teams cleanup cron: %s", err)
	}
}
