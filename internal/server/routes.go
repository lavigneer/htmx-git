package server

import (
	"html/template"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/gomarkdown/markdown"
	"github.com/microcosm-cc/bluemonday"
)

var templates = template.Must(template.ParseGlob("templates/*.tmpl.html"))

func (s *Server) RegisterRoutes() http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Logger)

	workDir, _ := os.Getwd()
	filesDir := http.Dir(filepath.Join(workDir, "assets"))
	FileServer(r, "/assets/", filesDir)

	r.Get("/", s.IndexHandler)
	r.Get("/commit/{sha}/file/*", s.CommitHandler)

	return r
}

// FileServer conveniently sets up a http.FileServer handler to serve
// static files from a http.FileSystem.
func FileServer(r chi.Router, path string, root http.FileSystem) {
	if strings.ContainsAny(path, "{}*") {
		panic("FileServer does not permit any URL parameters.")
	}

	if path != "/" && path[len(path)-1] != '/' {
		r.Get(path, http.RedirectHandler(path+"/", 301).ServeHTTP)
		path += "/"
	}
	path += "*"

	r.Get(path, func(w http.ResponseWriter, r *http.Request) {
		rctx := chi.RouteContext(r.Context())
		pathPrefix := strings.TrimSuffix(rctx.RoutePattern(), "/*")
		fs := http.StripPrefix(pathPrefix, http.FileServer(root))
		fs.ServeHTTP(w, r)
	})
}

func (s *Server) IndexHandler(w http.ResponseWriter, r *http.Request) {
	repo, err := git.PlainOpen("test-repo")
	if err != nil {
		log.Print("here")
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	ref, err := repo.Head()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	commitIter, err := repo.Log(&git.LogOptions{From: ref.Hash()})
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	lastestCommit, err := commitIter.Next()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	tree, err := lastestCommit.Tree()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	readme, err := tree.File("README.md")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	readmeContent, err := readme.Contents()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
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
	err = templates.ExecuteTemplate(w, "index.tmpl.html", data)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
}
