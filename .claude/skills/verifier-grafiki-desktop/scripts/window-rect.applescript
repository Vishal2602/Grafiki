-- Prints "x,y,w,h" of the Grafiki window, for `screencapture -R`.
tell application "System Events"
  tell (first application process whose name contains "grafiki")
    set p to position of window 1
    set s to size of window 1
    return (item 1 of p as string) & "," & (item 2 of p as string) & "," & (item 1 of s as string) & "," & (item 2 of s as string)
  end tell
end tell
