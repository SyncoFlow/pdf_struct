pdf_struct is a library allowing you to represent the structure of a PDF document into code by providing APIs to 
1. Define the structure of a document 
2. Serialize the structure of a physical document into the defined structure. 

The library will do this in parallel, with configuration for expected patterns in the document (i.e Type1 and Type2 are pairs that are always next to eachother in the document, and are children to Type3) 
With key types being specified to state a type is required, and wherever we infer one to be we have to actively check if it's there, otherwise we can apply other rules onto it failing 
For example, if Chapter is our key and a Chapter contains Diagrams and Tables until the next Chapter, we can assume the next two pages are a diagram-table pair, then check the third to be a Chapter, 
if it wasn't then we just check the page 5 pages ahead of the original Chapter. This will likely be improved, maybe with some runtime pattern-finding (i.e if we see 3 diagram-table pairs per Chapter on average 
we increase the number of pages that we infer to 6 from 2, although a very basic concept and would require much more functionality like being able to check a key on each page (i.e chapter number) to validate we didn't 
go past the next chapter page, and further increasing context. 

Currently, my idea for using parallelization will be assigning a thread to each page we process, 
with a classifer orchestrating each page being processed (i.e onto what type, what to do if it fails, applying previous context, etc) 

Although much of this is subject to change, since I'm not a 10x engineer and cannot plan out a medium sized project
completely from the beginning. 

Any public help will be utilized and is greatly appreciated!
