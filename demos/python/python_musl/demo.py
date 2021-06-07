import pandas as pd
import numpy as np
from sklearn.datasets import dump_svmlight_file

df1 = pd.read_csv("./dataset/input_label.csv")
df2 = pd.read_csv("./dataset/input.csv")
res = pd.merge(df1, df2, how='left', left_on='id', right_on='id')

X = res[np.setdiff1d(res.columns,['label','id'])]
y = res.label

dump_svmlight_file(X,y,'/host/smvlight.dat',zero_based=True,multilabel=False)
