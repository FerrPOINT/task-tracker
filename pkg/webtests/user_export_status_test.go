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

	apiv1 "github.com/FerrPOINT/task-tracker/pkg/routes/api/v1"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestUserExportStatus(t *testing.T) {
	t.Run("no export", func(t *testing.T) {
		rec, err := newTestRequestWithUser(t, http.MethodGet, apiv1.GetUserExportStatus, &testuser15, "", nil, nil)
		require.NoError(t, err)
		assert.Equal(t, "{}\n", rec.Body.String())
	})

	t.Run("with export", func(t *testing.T) {
		rec, err := newTestRequestWithUser(t, http.MethodGet, apiv1.GetUserExportStatus, &testuser1, "", nil, nil)
		require.NoError(t, err)
		assert.Contains(t, rec.Body.String(), `"id":1`)
	})
}
