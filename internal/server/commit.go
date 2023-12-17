package server

import (
	"html/template"
	"net/http"
	"sort"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/labstack/echo/v4"
)

var commitTemplates = template.Must(template.ParseFiles("templates/base.tmpl.html", "templates/filelist.tmpl.html"))
var fileTemplates = template.Must(template.ParseFiles("templates/base.tmpl.html", "templates/file.tmpl.html"))

func (s *Server) CommitHandler(c echo.Context) error {
	sha := c.Param("sha")
	path := c.Param("*")

	repo, err := git.PlainOpen("test-repo")
	if err != nil {
		return err
	}
	commitHash := plumbing.NewHash(sha)
	commitIter, err := repo.Log(&git.LogOptions{From: commitHash})
	if err != nil {
		return err
	}

	commit, err := commitIter.Next()
	if err != nil {
		return err
	}

	tree, err := commit.Tree()
	if err != nil {
		return err
	}

	entryAtPath, err := tree.FindEntry(path)
	if err != nil {
		return err
	} else if entryAtPath.Mode.IsFile() {
		file, err := tree.File(path)
		if err != nil {
			return err
		}
		fileContent, err := file.Contents()
		if err != nil {
			return err
		}
		return c.Render(http.StatusOK, "file", fileContent)

	}

	tree, err = tree.Tree(path)
	if err != nil {
		return err
	}
	treeEntries := tree.Entries
	sort.Slice(treeEntries, func(i, j int) bool {
		if treeEntries[i].Mode.IsFile() != treeEntries[j].Mode.IsFile() {
			if treeEntries[i].Mode.IsFile() {
				return false
			}
			return true
		}
		return treeEntries[i].Name < treeEntries[j].Name
	})

	data := struct {
		TreeEntries []object.TreeEntry
		CommitHash  plumbing.Hash
		Path        string
	}{
		TreeEntries: treeEntries,
		CommitHash:  commitHash,
		Path:        path + "/",
	}
	return c.Render(http.StatusOK, "filelist", data)
}
