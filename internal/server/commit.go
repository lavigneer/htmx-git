package server

import (
	"html/template"
	"log"
	"net/http"
	"sort"

	"github.com/go-chi/chi/v5"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
)

var commitTemplates = template.Must(template.ParseFiles("templates/base.tmpl.html", "templates/filelist.tmpl.html"))
var fileTemplates = template.Must(template.ParseFiles("templates/base.tmpl.html", "templates/file.tmpl.html"))

func (s *Server) CommitHandler(w http.ResponseWriter, r *http.Request) {
	sha := chi.URLParam(r, "sha")
	path := chi.URLParam(r, "*")

	repo, err := git.PlainOpen("test-repo")
	if err != nil {
		log.Print("here")
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	commitHash := plumbing.NewHash(sha)
	commitIter, err := repo.Log(&git.LogOptions{From: commitHash})
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	commit, err := commitIter.Next()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	tree, err := commit.Tree()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	entryAtPath, err := tree.FindEntry(path)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	} else if entryAtPath.Mode.IsFile() {
		file, err := tree.File(path)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		fileContent, err := file.Contents()
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		err = fileTemplates.ExecuteTemplate(w, "file.tmpl.html", fileContent)

		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		return

	}

	tree, err = tree.Tree(path)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
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
	err = commitTemplates.ExecuteTemplate(w, "filelist.tmpl.html", data)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
}
