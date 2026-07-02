on run argv
  set targetName to item 1 of argv
  tell application "System Events"
    tell (first application process whose name contains "grafiki")
      set frontmost to true
      delay 0.3
      set allEls to entire contents of window 1
      repeat with el in allEls
        try
          if class of el is button then
            if (name of el is targetName) then
              click el
              return "clicked button: " & targetName
            end if
          end if
        end try
      end repeat
      return "NOT FOUND: " & targetName
    end tell
  end tell
end run
