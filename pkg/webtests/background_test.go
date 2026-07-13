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

package webtests

import (
	"net/http"
	"testing"

	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/models"
	bgHandler "github.com/FerrPOINT/task-tracker/pkg/modules/background/handler"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestRemoveProjectBackgroundPreservesTitle(t *testing.T) {
	t.Run("Deleting background does not clear project title", func(t *testing.T) {
		// testuser6 owns project 35, which has:
		//   title: "Test35 with background"
		//   background_file_id: 1
		_, err := newTestRequestWithUser(
			t,
			http.MethodDelete,
			bgHandler.RemoveProjectBackground,
			&testuser6,
			"",
			nil,
			map[string]string{"project": "35"},
		)
		require.NoError(t, err)

		// Verify the title is preserved by querying the DB directly
		s := db.NewSession()
		defer s.Close()

		project := models.Project{ID: 35}
		has, err := s.Get(&project)
		require.NoError(t, err)
		assert.True(t, has)
		assert.Equal(t, "Test35 with background", project.Title)
		assert.Equal(t, int64(0), project.BackgroundFileID)
	})
}

func TestProjectBackgroundDeletePermission(t *testing.T) {
	t.Run("Read-only user cannot delete project background", func(t *testing.T) {
		// testuser15 has read-only access (permission: 0) to project 35,
		// which has background_file_id: 1.
		// Deleting the background should require write access.
		_, err := newTestRequestWithUser(
			t,
			http.MethodDelete,
			bgHandler.RemoveProjectBackground,
			&testuser15,
			"",
			nil,
			map[string]string{"project": "35"},
		)

		// Should be forbidden for a read-only user
		require.Error(t, err)
		assert.Equal(t, http.StatusForbidden, getHTTPErrorCode(err))
	})
}
