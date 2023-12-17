package server

import (
	"html/template"
	"log"
	"net/http"
	"sort"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/gomarkdown/markdown"
	"github.com/microcosm-cc/bluemonday"
)

var templates = template.Must(template.ParseGlob("templates/*.tmpl.html"))

func (s *Server) RegisterRoutes() http.Handler {
	assetsHandler := http.StripPrefix("/assets/", http.FileServer(http.Dir("assets")))

	mux := http.NewServeMux()
	mux.Handle("/assets/", assetsHandler)
	mux.HandleFunc("/commit/", s.CommitHandler)
	mux.HandleFunc("/", s.IndexHandler)

	return mux
}

func (s *Server) CommitHandler(w http.ResponseWriter, r *http.Request) {
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
		Reference     *plumbing.Reference
		Path          string
	}{
		ReadmeContent: template.HTML(bluemonday.UGCPolicy().SanitizeBytes(readmeContentBytes)),
		TreeEntries:   tree.Entries,
		Reference:     ref,
		Path:          "",
	}
	err = templates.ExecuteTemplate(w, "index.tmpl.html", data)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
}
