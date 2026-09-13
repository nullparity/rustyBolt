# Command line

```sh
rustybolt                    # open the desktop window
rustybolt --browser          # open the launcher in your default browser
rustybolt launch runelite    # start RuneLite
rustybolt launch hdos        # start HDOS
rustybolt login              # log in to your Jagex account once
rustybolt verify             # show what a launch does without starting anything
rustybolt configure          # open the settings page
rustybolt help               # show usage
```

`rustybolt login` prints a URL. Open it in a browser and log in. The browser lands on a redirect page. Copy the whole address bar of that page and paste it at the prompt. The launcher saves the session, so later launches need no login.

`rustybolt verify` checks the client jar, the session and the Java runtime, then prints the command line that `launch` runs.

`rustybolt launch runelite --jar <path>` starts a jar in a non-standard place.
