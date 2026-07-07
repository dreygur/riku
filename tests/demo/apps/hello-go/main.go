package main

import (
	"fmt"
	"net/http"
	"os"
)

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprintf(w, "hello from hello-go (pid %d)\n", os.Getpid())
	})
	http.ListenAndServe("0.0.0.0:"+port, nil)
}
