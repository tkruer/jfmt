package com.example;

import java.util.*; // wildcard import
import static java.lang.System.*; // static import (ok)

public class BadExample {
	public static void main(String[] args) {
		; // empty statement
		out.println("This is a very long line that should definitely exceed one hundred characters in length to trigger the max line length rule 1234567890");
		if (true) { ; } // empty statement inside block
	}
}

