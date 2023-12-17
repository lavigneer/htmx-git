package server

import (
	"html/template"
	"log"
	"net/http"

	"github.com/go-git/go-git/v5"
	"github.com/gomarkdown/markdown"
	"github.com/microcosm-cc/bluemonday"
)

var templates = template.Must(template.ParseGlob("templates/*.tmpl.html"))

func (s *Server) RegisterRoutes() http.Handler {
	assetsHandler := http.StripPrefix("/assets/", http.FileServer(http.Dir("assets")))

	mux := http.NewServeMux()
	mux.Handle("/assets/", assetsHandler)
	mux.HandleFunc("/", s.IndexHandler)

	return mux
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

	data := struct {
		ReadmeContent template.HTML
	}{
		ReadmeContent: template.HTML(bluemonday.UGCPolicy().SanitizeBytes(readmeContentBytes)),
	}

	err = templates.ExecuteTemplate(w, "index.tmpl.html", data)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
}
