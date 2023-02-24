import os
import json

# Directory path
directory = "./database/users/"

# Iterate through all files in the directory
for filename in os.listdir(directory):
    if filename.endswith(".json"):
        # Open the file and load the JSON data
        with open(os.path.join(directory, filename), "r") as f:
            data = json.load(f)
        
        # Add a new field to the JSON data
        data["flight"] = None
        
        # Write the updated JSON data back to the file
        with open(os.path.join(directory, filename), "w") as f:
            json.dump(data, f, indent=4)
