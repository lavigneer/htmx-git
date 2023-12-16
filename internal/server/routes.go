package server

import (
	"html/template"
	"net/http"
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
	err := templates.ExecuteTemplate(w, "index.tmpl.html", nil)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}
