package server

import (
	"html/template"
	"net/http"
	"sort"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/gomarkdown/markdown"
	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
	"github.com/microcosm-cc/bluemonday"
)

func (s *Server) RegisterRoutes() http.Handler {
	e := echo.New()
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())

	templateRegistry := s.NewTemplateRegistry()
	e.Renderer = &templateRegistry

	e.Static("/assets", "assets")
	e.GET("/", s.IndexHandler)
	e.GET("/commit/:sha/file/*", s.CommitHandler)

	return e
}

func (s *Server) IndexHandler(c echo.Context) error {
	repo, err := git.PlainOpen("test-repo")
	if err != nil {
		return err
	}

	ref, err := repo.Head()
	if err != nil {
		return err
	}

	commitIter, err := repo.Log(&git.LogOptions{From: ref.Hash()})
	if err != nil {
		return err
	}

	lastestCommit, err := commitIter.Next()
	if err != nil {
		return err
	}

	tree, err := lastestCommit.Tree()
	if err != nil {
		return err
	}

	readme, err := tree.File("README.md")
	if err != nil {
		return err
	}

	readmeContent, err := readme.Contents()
	if err != nil {
		return err
	}

	readmeContentBytes := markdown.ToHTML([]byte(readmeContent), nil, nil)

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
		ReadmeContent template.HTML
		TreeEntries   []object.TreeEntry
		CommitHash    plumbing.Hash
		Path          string
	}{
		ReadmeContent: template.HTML(bluemonday.UGCPolicy().SanitizeBytes(readmeContentBytes)),
		TreeEntries:   tree.Entries,
		CommitHash:    ref.Hash(),
		Path:          "",
	}
	return c.Render(http.StatusOK, "index", data)
}
