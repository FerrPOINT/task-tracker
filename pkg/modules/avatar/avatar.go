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

package avatar

import (
	"errors"
	"image"
	"io"
	"strings"

	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/botmarble"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/empty"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/gravatar"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/initials"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/ldap"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/marble"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/openid"
	"github.com/FerrPOINT/task-tracker/pkg/modules/avatar/upload"
	"github.com/FerrPOINT/task-tracker/pkg/user"

	"github.com/gabriel-vasile/mimetype"
	"xorm.io/xorm"
)

// ErrNotAnImage is returned by StoreUploadedAvatar when the uploaded file is not an image.
var ErrNotAnImage = errors.New("uploaded file is no image")

// Provider defines the avatar provider interface
type Provider interface {
	// GetAvatar is the method used to get an actual avatar for a user
	GetAvatar(user *user.User, size int64) (avatar []byte, mimeType string, err error)
	// AsDataURI returns a base64-encoded string representation of the avatar suitable for inline use
	AsDataURI(user *user.User, size int64) (inlineData string, err error)
	// FlushCache removes cached avatar data for the user
	FlushCache(u *user.User) error
}

// FlushAllCaches removes cached avatars for the given user for all providers
func FlushAllCaches(u *user.User) {
	providers := []Provider{
		&upload.Provider{},
		&gravatar.Provider{},
		&initials.Provider{},
		&ldap.Provider{},
		&openid.Provider{},
		&marble.Provider{},
		&botmarble.Provider{},
		&empty.Provider{},
	}
	for _, p := range providers {
		if err := p.FlushCache(u); err != nil {
			log.Errorf("Error flushing avatar cache: %v", err)
		}
	}
}

// GetAvatarForUsername resolves and renders the avatar for a username. It is the
// shared core behind both the v1 and v2 avatar endpoints: it looks up the user,
// tolerates an unknown/disabled user (returning the default placeholder rather
// than an error, since avatars are loaded via <img> tags), picks the right
// provider (empty for unknown users, botmarble for bots, otherwise the user's
// configured provider) and clamps the size to the server's configured maximum.
func GetAvatarForUsername(s *xorm.Session, username string, size int64) (data []byte, mime string, err error) {
	u, err := user.GetUserWithEmail(s, &user.User{Username: username})
	if err != nil && !user.IsErrUserDoesNotExist(err) && !user.IsErrUserStatusError(err) {
		log.Errorf("Error getting user for avatar: %v", err)
		return nil, "", err
	}

	found := err == nil || user.IsErrUserStatusError(err)

	provider := GetProvider(u)
	if !found {
		// Unknown user: serve the default placeholder.
		provider = &empty.Provider{}
	}
	if found && u.IsBot() {
		provider = &botmarble.Provider{}
	}

	if size > config.ServiceMaxAvatarSize.GetInt64() {
		size = config.ServiceMaxAvatarSize.GetInt64()
	}

	data, mime, err = provider.GetAvatar(u, size)
	if err != nil {
		log.Errorf("Error getting avatar for user %d: %v", u.ID, err)
		return nil, "", err
	}

	return data, mime, nil
}

// GetProvider returns the appropriate avatar provider for a user
func GetProvider(u *user.User) Provider {
	provider := u.AvatarProvider
	if provider == "" {
		provider = "empty"
	}

	switch provider {
	case "gravatar":
		return &gravatar.Provider{}
	case "initials":
		return &initials.Provider{}
	case "upload":
		return &upload.Provider{}
	case "marble":
		return &marble.Provider{}
	case "ldap":
		return &ldap.Provider{}
	case "openid":
		return &openid.Provider{}
	default:
		return &empty.Provider{}
	}
}

// StoreUploadedAvatar validates that src is an image, switches the user's avatar
// provider to "upload", stores the image as the user's avatar and flushes all
// cached avatars for the user. It returns ErrNotAnImage if src is not an image.
func StoreUploadedAvatar(s *xorm.Session, u *user.User, src io.ReadSeeker) error {
	mime, err := mimetype.DetectReader(src)
	if err != nil {
		return err
	}
	if !strings.HasPrefix(mime.String(), "image") {
		return ErrNotAnImage
	}
	if _, err := src.Seek(0, io.SeekStart); err != nil {
		return err
	}

	// The mimetype sniff above accepts image types we cannot actually store
	// (e.g. SVG, WebP) because upload.StoreAvatarFile decodes via image.Decode,
	// which only has the decoders registered process-wide by the imaging package
	// (png, jpeg, gif, tiff, bmp). image.DecodeConfig uses those same decoders, so
	// validating here rejects undecodable images with a 400 instead of failing
	// deeper in storage with a 500.
	if _, _, err := image.DecodeConfig(src); err != nil {
		return ErrNotAnImage
	}
	if _, err := src.Seek(0, io.SeekStart); err != nil {
		return err
	}

	u.AvatarProvider = "upload"
	if err := upload.StoreAvatarFile(s, u, src); err != nil {
		return err
	}

	FlushAllCaches(u)

	return nil
}
