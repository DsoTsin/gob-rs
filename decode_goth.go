package main

import (
	"encoding/gob"
	"fmt"
	"log"
	"os"
	"reflect"

	"github.com/gorilla/sessions"
)

func main() {
	fmt.Println("Starting decode_goth.go...")
	// Open the file
	file, err := os.Open("goth-session.bin")
	if err != nil {
		log.Fatalf("Error opening file: %v", err)
	}
	defer file.Close()

	// Register likely types
	gob.Register(&sessions.Session{})
	gob.Register(&sessions.Options{}) // Added Options explicitly
	gob.Register(map[string]interface{}{})
	gob.Register(map[interface{}]interface{}{})

	// Create a decoder
	decoder := gob.NewDecoder(file)

	var data map[interface{}]interface{}

	if err := decoder.Decode(&data); err != nil {
		log.Printf("Decode error: %v", err)
		return
	}

	fmt.Printf("Decoded Data: %#v\n", data)
	printDetails(data, "")
}

func printDetails(data interface{}, indent string) {
	val := reflect.ValueOf(data)
	if val.Kind() == reflect.Ptr {
		val = val.Elem()
	}

	if !val.IsValid() {
		fmt.Println(indent + "nil")
		return
	}

	switch val.Kind() {
	case reflect.Map:
		fmt.Println(indent + "Map:")
		iter := val.MapRange()
		for iter.Next() {
			k := iter.Key()
			v := iter.Value()
			fmt.Printf("%sKey: %v (%T)\n", indent+"  ", k.Interface(), k.Interface())
			fmt.Printf("%sValue: (%T)\n", indent+"  ", v.Interface())
			printDetails(v.Interface(), indent+"    ")
		}
	case reflect.Slice, reflect.Array:
		fmt.Println(indent + "Slice/Array:")
		for i := 0; i < val.Len(); i++ {
			fmt.Printf("%sIndex %d:\n", indent+"  ", i)
			printDetails(val.Index(i).Interface(), indent+"    ")
		}
	case reflect.Struct:
		fmt.Println(indent + "Struct " + val.Type().Name() + ":")
		for i := 0; i < val.NumField(); i++ {
			field := val.Type().Field(i)
			// Skip unexported fields
			if field.PkgPath != "" {
				continue
			}
			fmt.Printf("%sField %s (%s):\n", indent+"  ", field.Name, field.Type)
			printDetails(val.Field(i).Interface(), indent+"    ")
		}
	default:
		fmt.Printf("%s%v (%T)\n", indent, data, data)
	}
}
