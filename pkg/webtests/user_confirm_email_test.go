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
	"github.com/FerrPOINT/task-tracker/pkg/user"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestUserConfirmEmail(t *testing.T) {
	t.Run("Normal test", func(t *testing.T) {
		rec, err := newTestRequest(t, http.MethodPost, apiv1.UserConfirmEmail, `{"token": "tiepiQueed8ahc7zeeFe1eveiy4Ein8osooxegiephauph2Ael"}`, nil, nil)
		require.NoError(t, err)
		assert.Contains(t, rec.Body.String(), `The email was confirmed successfully.`)
	})
	t.Run("Empty payload", func(t *testing.T) {
		_, err := newTestRequest(t, http.MethodPost, apiv1.UserConfirmEmail, `{}`, nil, nil)
		require.Error(t, err)
		assert.Equal(t, http.StatusPreconditionFailed, getHTTPErrorCode(err))
		assertHandlerErrorCode(t, err, user.ErrCodeInvalidEmailConfirmToken)
	})
	t.Run("Empty token", func(t *testing.T) {
		_, err := newTestRequest(t, http.MethodPost, apiv1.UserConfirmEmail, `{"token": ""}`, nil, nil)
		require.Error(t, err)
		assertHandlerErrorCode(t, err, user.ErrCodeInvalidEmailConfirmToken)
	})
	t.Run("Invalid token", func(t *testing.T) {
		_, err := newTestRequest(t, http.MethodPost, apiv1.UserConfirmEmail, `{"token": "invalidToken"}`, nil, nil)
		require.Error(t, err)
		assertHandlerErrorCode(t, err, user.ErrCodeInvalidEmailConfirmToken)
	})
}
