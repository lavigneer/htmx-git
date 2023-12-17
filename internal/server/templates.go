package server

import (
	"errors"
	"html/template"
	"io"

	"github.com/labstack/echo/v4"
)

type TemplateRegistry struct {
	templates map[string]*template.Template
}

// Implement e.Renderer interface
func (t *TemplateRegistry) Render(w io.Writer, name string, data interface{}, c echo.Context) error {
	tmpl, ok := t.templates[name]
	if !ok {
		err := errors.New("Template not found -> " + name)
		return err
	}
	return tmpl.ExecuteTemplate(w, "base", data)
}

func (s *Server) NewTemplateRegistry() TemplateRegistry {
	templates := make(map[string]*template.Template)
	templates["index"] = template.Must(template.ParseFiles("templates/filelist.tmpl.html", "templates/index.tmpl.html", "templates/base.tmpl.html"))
	templates["filelist"] = template.Must(template.ParseFiles("templates/filelist.tmpl.html", "templates/base.tmpl.html"))
	templates["file"] = template.Must(template.ParseFiles("templates/file.tmpl.html", "templates/base.tmpl.html"))
	return TemplateRegistry{
		templates: templates,
	}
}
